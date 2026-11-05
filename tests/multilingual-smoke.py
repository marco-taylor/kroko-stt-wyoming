#!/usr/bin/env python3
"""Isolated native tests with copied models. Never downloads or mounts production.

Optional --english supplies already authorized/downloaded EN packages. Without it,
language switching must fail safely instead of reusing German weights.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import socket
import subprocess
import time
import uuid
import wave


def run(args, check=True):
    p = subprocess.run(args, capture_output=True, text=True, timeout=240)
    if check and p.returncode:
        raise RuntimeError(p.stderr[-3000:] + p.stdout[-2000:])
    return p


def digest(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def tree(path):
    return {str(p.relative_to(path)): [digest(p), p.stat().st_mtime_ns]
            for p in path.rglob('*') if p.is_file()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--image', required=True)
    ap.add_argument('--binaries', type=pathlib.Path)
    ap.add_argument('--models', type=pathlib.Path, required=True)
    ap.add_argument('--audio', type=pathlib.Path, required=True)
    ap.add_argument('--english', type=pathlib.Path)
    ap.add_argument('--english-audio', type=pathlib.Path)
    ap.add_argument('--output', type=pathlib.Path, required=True)
    a = ap.parse_args()
    a.output = a.output.resolve()
    a.output.mkdir(exist_ok=False)
    root = a.output / 'models'
    root.mkdir()
    os.chown(root, 99, 100)
    for n in [64, 128]:
        d = root / f'de/community-{n}'
        d.mkdir(parents=True)
        os.chown(d, 99, 100)
        package = f'Kroko-DE-Community-{n}-L-Streaming-001.data'
        shutil.copyfile(a.models / f'community-{n}' / package, d / package)
    prefix = 'kroko-multilingual-test-' + uuid.uuid4().hex[:10]
    names = []
    results = {}

    def options(language='de', model='community-128', ro=False):
        args = ['--read-only', '--user', '99:100', '--cap-drop', 'ALL',
                '--security-opt', 'no-new-privileges', '--mount',
                f'type=bind,src={root},dst=/models' + (',readonly' if ro else ''),
                '--mount', f'type=bind,src={a.audio.resolve()},dst=/test/01.wav,readonly',
                '--env', 'LANGUAGE=' + language, '--env', 'MODEL=' + model,
                '--env', 'MODEL_AUTO_DOWNLOAD=false', '--env', 'NUM_THREADS=1',
                '--env', 'LOG_LEVEL=info']
        if a.binaries:
            for binary in ['kroko-wyoming', 'model-manager', 'wyoming-probe']:
                args += ['--mount', f'type=bind,src={a.binaries.resolve()/binary},dst=/usr/local/bin/{binary},readonly']
        return args

    def cli(*args, check=True, **kwargs):
        return run(['docker', 'run', '--rm', '--network', 'none', *options(**kwargs),
                    '--entrypoint', '/usr/local/bin/model-manager', a.image, *args], check)

    def stop(name):
        run(['docker', 'stop', '--time', '5', name])
        run(['docker', 'rm', name])
        names.remove(name)

    def start(label, offline=False, **kwargs):
        name = prefix + '-' + label
        names.append(name)
        network = ['--network', 'none'] if offline else ['--network', 'bridge', '-p', '127.0.0.1::10321']
        run(['docker', 'run', '-d', '--name', name, *network, *options(**kwargs), a.image])
        deadline = time.monotonic() + 150
        while time.monotonic() < deadline:
            if run(['docker', 'exec', name, '/usr/local/bin/wyoming-probe'], False).returncode == 0:
                native = run(['docker', 'logs', name], False)
                events = []
                for line in (native.stdout + native.stderr).splitlines():
                    try:
                        item = json.loads(line)
                        if item.get('event') == 'model_loaded':
                            events.append(item)
                    except (ValueError, AttributeError):
                        pass
                assert len(events) == 1, 'Exactly one recognizer must be initialized'
                assert events[0]['identity'] == kwargs.get('language', 'de') + '/' + kwargs.get('model', 'community-128')
                results.setdefault('single_model_loads', []).append({
                    'label': label, 'identity': events[0]['identity'],
                    'loads': len(events), 'rss_kib': events[0].get('rss_kib')})
                return name
            state = run(['docker', 'inspect', name, '--format', '{{.State.Running}}']).stdout.strip()
            if state != 'true':
                raise RuntimeError(run(['docker', 'logs', name], False).stderr)
            time.sleep(.5)
        raise RuntimeError('Model startup timeout')

    def transcribe(name, language='de', audio=None):
        info = json.loads(run(['docker', 'inspect', name]).stdout)[0]
        port = int(info['NetworkSettings']['Ports']['10321/tcp'][0]['HostPort'])
        assert port != 10321
        with socket.create_connection(('127.0.0.1', port), timeout=30) as s:
            stream = s.makefile('rb')

            def send(kind, data=None, payload=b''):
                h = {'type': kind, 'data': data or {}}
                if payload:
                    h['payload_length'] = len(payload)
                s.sendall(json.dumps(h).encode() + b'\n' + payload)

            send('describe')
            description = json.loads(stream.readline())
            assert description['type'] == 'info'
            languages = description['data']['asr'][0]['models'][0]['languages']
            assert language in languages
            send('transcribe', {'language': language})
            fmt = {'rate': 16000, 'width': 2, 'channels': 1}
            send('audio-start', fmt)
            with wave.open(str(audio or a.audio), 'rb') as wav:
                assert (wav.getframerate(), wav.getsampwidth(), wav.getnchannels()) == (16000, 2, 1)
                while chunk := wav.readframes(320):
                    send('audio-chunk', fmt, chunk)
            send('audio-stop')
            result = json.loads(stream.readline())
            assert result['type'] == 'transcript', result
            text = result['data']['text']
            assert text.strip() and result['data']['language'] == language
            if language == 'de':
                assert text.lower().strip().rstrip('.!?') == 'schalte das licht im wohnzimmer ein'
            return {'host_port': port, 'languages': languages, 'text': text}

    try:
        unselected = tree(root / 'de/community-128')
        name = start('de64', model='community-64')
        results['de64'] = transcribe(name)
        assert not list((root / 'de/community-64').glob('*.data'))
        assert tree(root / 'de/community-128') == unselected
        stop(name)
        first64 = tree(root / 'de/community-64')
        name = start('de128', model='community-128')
        results['switch_64_to_128'] = transcribe(name)
        assert not list((root / 'de/community-128').glob('*.data'))
        assert tree(root / 'de/community-64') == first64
        stop(name)
        prepared = tree(root)
        name = start('de64-again', model='community-64', ro=True)
        results['switch_128_to_64'] = transcribe(name)
        stop(name)
        assert tree(root) == prepared
        name = start('offline128', model='community-128', ro=True, offline=True)
        p = run(['docker', 'exec', name, '/usr/local/bin/wyoming-probe',
                 '--smoke-fast', '127.0.0.1:10321', '/test/01.wav'])
        results['offline_readonly'] = [json.loads(line) for line in p.stdout.splitlines()]
        stop(name)
        assert tree(root) == prepared
        en = json.loads(cli('status', language='en').stdout)
        assert not en['present'] and en['identity'] == 'en/community-128'
        assert cli('install', 'classic', language='en', check=False).returncode
        assert cli('status', language='zz', check=False).returncode
        assert cli('status', language='sv', check=False).returncode
        p = run(['docker', 'run', '--rm', '--network', 'none', *options(language='en'), a.image], False)
        assert p.returncode and not (root / 'en/community-128').exists()
        results['missing_en_never_reuses_de'] = True
        if a.english:
            de_before = tree(root / 'de')
            for n in [64, 128]:
                d = root / f'en/community-{n}'
                d.mkdir(parents=True)
                os.chown(d, 99, 100)
                source = a.english / f'Kroko-EN-Community-{n}-L-Streaming-001.data'
                shutil.copyfile(source, d / source.name)
                name = start('en' + str(n), language='en', model='community-' + str(n))
                results['en' + str(n)] = transcribe(name, 'en', a.english_audio)
                stop(name)
                assert not list(d.glob('*.data'))
            assert tree(root / 'de') == de_before
            record_path = next((root / 'en/community-128').rglob('installation.json'))
            original_record = record_path.read_bytes()
            record = json.loads(original_record);record.pop('language')
            record_path.write_text(json.dumps(record))
            try:
                assert cli('verify', 'community-128', language='en', check=False).returncode
                results['missing_language_legacy_exception_is_german_only'] = True
            finally:
                record_path.write_bytes(original_record)
        else:
            results['en_native'] = {'skipped': 'Full English packages not supplied; no unapproved download'}
        results['all_passed'] = True
    finally:
        for name in names:
            if name.startswith(prefix):
                run(['docker', 'rm', '--force', name], False)
        (a.output / 'results.json').write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
