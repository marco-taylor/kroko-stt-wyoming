"""Isolated candidate-container utilities. No image builds or production mounts."""
import asyncio
import json
import os
import pathlib
import socket
import subprocess
import time
import uuid
import wave


def command(args, check=True):
    p = subprocess.run(args, capture_output=True, text=True, timeout=240)
    if check and p.returncode:
        raise RuntimeError(p.stderr[-3000:] + p.stdout[-2000:])
    return p


def process_sample(pid):
    p = pathlib.Path('/proc') / str(pid)
    s = p.joinpath('stat').read_text()
    fields = s[s.rindex(')') + 2:].split()
    values = dict(line.split(':', 1) for line in p.joinpath('status').read_text().splitlines() if ':' in line)
    return {'cpu_s': (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK'),
            'rss_bytes': int(values['VmRSS'].split()[0]) * 1024,
            'hwm_bytes': int(values['VmHWM'].split()[0]) * 1024}


async def event(reader):
    header = json.loads(await reader.readline())
    data = header.get('data', {})
    if header.get('data_length', 0):
        data.update(json.loads(await reader.readexactly(header['data_length'])))
    if header.get('payload_length', 0):
        await reader.readexactly(header['payload_length'])
    return header['type'], data


async def send(writer, kind, data=None, payload=b''):
    data = json.dumps(data or {}, ensure_ascii=False).encode()
    header = {'type': kind, 'data_length': len(data), 'payload_length': len(payload)}
    writer.write(json.dumps(header).encode() + b'\n' + data + payload)
    await writer.drain()


async def transcribe(port, path, paced=False, language='de-DE', timeout=180):
    async def job():
        started = time.perf_counter()
        reader, writer = await asyncio.open_connection('127.0.0.1', port)
        eof = None
        try:
            await send(writer, 'transcribe', {'language': language})
            fmt = {'rate': 16000, 'width': 2, 'channels': 1}
            await send(writer, 'audio-start', fmt)

            async def upload():
                nonlocal eof
                with wave.open(str(path)) as wav:
                    assert (wav.getframerate(), wav.getsampwidth(), wav.getnchannels()) == (16000, 2, 1)
                    frames = 0
                    audio_start = time.perf_counter()
                    while chunk := wav.readframes(320):
                        frames += len(chunk) // 2
                        if paced:
                            await asyncio.sleep(max(0, audio_start + frames / 16000 - time.perf_counter()))
                        await send(writer, 'audio-chunk', fmt, chunk)
                await send(writer, 'audio-stop')
                eof = time.perf_counter()

            uploader = asyncio.create_task(upload())
            try:
                while True:
                    kind, data = await event(reader)
                    if kind == 'error':
                        raise RuntimeError(str(data))
                    if kind == 'transcript':
                        ended = time.perf_counter()
                        assert eof is not None, 'Premature transcript'
                        await uploader
                        return {'hypothesis': data['text'], 'language': data.get('language'),
                                'total_s': ended - started, 'latency_s': ended - eof}
            finally:
                if not uploader.done():
                    uploader.cancel()
                await asyncio.gather(uploader, return_exceptions=True)
        finally:
            writer.close()
            await writer.wait_closed()
    return await asyncio.wait_for(job(), timeout)


class Candidate:
    def __init__(self, image, binaries, models, model, output, offline=False, debug=False, threads=1):
        self.name = 'kroko-nemo-test-' + uuid.uuid4().hex[:12]
        self.output = pathlib.Path(output)
        self.output.mkdir(parents=True, exist_ok=False)
        self.image, self.binaries, self.models, self.model = image, pathlib.Path(binaries), pathlib.Path(models), model
        if threads not in (1,2,4): raise ValueError("Test thread count must be 1, 2 or 4")
        self.threads = threads
        self.offline, self.debug = offline, debug
        self.port = self.pid = None

    def start(self):
        args = ['docker', 'run', '-d', '--name', self.name, '--read-only', '--user', '99:100',
                '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges', '--cpuset-cpus', {1:'3',2:'2,3',4:'0-3'}[self.threads],
                '--memory', '4g', '--network', 'none' if self.offline else 'bridge',
                '--mount', f'type=bind,src={self.models.resolve()},dst=/models,readonly',
                '--env', f'MODEL={self.model}', '--env', 'LANGUAGE=de-DE',
                '--env', 'MODEL_AUTO_DOWNLOAD=false', '--env', f'NUM_THREADS={self.threads}',
                '--env', 'LOG_LEVEL=debug' if self.debug else 'LOG_LEVEL=info']
        for name in ('kroko-wyoming', 'model-manager', 'wyoming-probe'):
            args += ['--mount', f'type=bind,src={(self.binaries/name).resolve()},dst=/usr/local/bin/{name},readonly']
        if not self.offline:
            args += ['-p', '127.0.0.1::10321']
        started = time.monotonic()
        command(args + [self.image])
        try:
            self.ready()
            self.startup_s = time.monotonic() - started
            return self
        except BaseException:
            self.close()
            raise

    def ready(self):
        end = time.monotonic() + 150
        while time.monotonic() < end:
            info = json.loads(command(['docker', 'inspect', self.name]).stdout)[0]
            if not info['State']['Running']:
                raise RuntimeError(command(['docker', 'logs', self.name], False).stderr[-4000:])
            if command(['docker', 'exec', self.name, '/usr/local/bin/wyoming-probe'], False).returncode == 0:
                self.pid = info['State']['Pid']
                if not self.offline:
                    self.port = int(info['NetworkSettings']['Ports']['10321/tcp'][0]['HostPort'])
                    assert self.port != 10321
                self.info = info
                return
            time.sleep(.3)
        raise TimeoutError('Candidate readiness timeout')

    def logs(self):
        p = command(['docker', 'logs', self.name], False)
        return p.stdout + p.stderr

    def close(self):
        command(['docker', 'stop', '--time', '20', self.name], False)
        text = self.logs()
        (self.output / 'server.log').write_text(text)
        command(['docker', 'rm', self.name], False)

    def restart(self):
        command(['docker', 'restart', '--time', '20', self.name])
        self.ready()

    async def describe(self):
        reader, writer = await asyncio.open_connection('127.0.0.1', self.port)
        try:
            await send(writer, 'describe')
            kind, data = await event(reader)
            assert kind == 'info'
            return data
        finally:
            writer.close()
            await writer.wait_closed()
