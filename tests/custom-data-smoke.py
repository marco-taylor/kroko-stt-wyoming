#!/usr/bin/env python3
"""Local-only custom-package checks. No downloads; never mounts production data.

Requires existing official Community-64/128 packages and a PCM16/16k/mono WAV
containing 'Schalte das Licht im Wohnzimmer ein'. All servers use unique names.
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
    p = subprocess.run(args, capture_output=True, text=True, timeout=180)
    if check and p.returncode:
        raise RuntimeError(p.stderr[-2000:] + p.stdout[-2000:])
    return p


def digest(path):
    with path.open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def tree(path):
    return {str(p.relative_to(path)): (digest(p), p.stat().st_mtime_ns)
            for p in path.rglob('*') if p.is_file()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--image', required=True)
    ap.add_argument('--binaries', type=pathlib.Path)
    ap.add_argument('--models', type=pathlib.Path, required=True)
    ap.add_argument('--audio', type=pathlib.Path, required=True)
    ap.add_argument('--output', type=pathlib.Path, required=True)
    a = ap.parse_args()
    a.output = a.output.resolve()
    a.output.mkdir(exist_ok=False)
    root = a.output / 'models'
    root.mkdir()
    os.chown(root, 99, 100)
    original_hashes = {}
    for n in [64, 128]:
        source = a.models / f'community-{n}/Kroko-DE-Community-{n}-L-Streaming-001.data'
        original_hashes[str(source)] = digest(source)
        shutil.copyfile(source, root / f'selected-{n}.data')
    (root / 'unrelated.keep').write_text('preserve other appdata')
    names = []
    prefix = 'kroko-custom-test-' + uuid.uuid4().hex[:10]
    results = {}

    def options(base=root, ro=False, model='custom/local', path='/models/selected-128.data'):
        overrides=[]
        if a.binaries:
            for binary in ['kroko-wyoming','model-manager','wyoming-probe']:
                overrides+=['--mount',f'type=bind,src={a.binaries.resolve()/binary},dst=/usr/local/bin/{binary},readonly']
        return overrides+['--read-only', '--user', '99:100', '--cap-drop', 'ALL',
                '--security-opt', 'no-new-privileges', '--mount',
                f'type=bind,src={base},dst=/models' + (',readonly' if ro else ''),
                '--mount', f'type=bind,src={a.audio.resolve()},dst=/test/01.wav,readonly',
                '--env', 'KROKO_MODEL=' + model, '--env', 'CUSTOM_MODEL_PATH=' + path,
                '--env', 'NUM_THREADS=1', '--env', 'LOG_LEVEL=warn']

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
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            p = run(['docker', 'exec', name, '/usr/local/bin/wyoming-probe'], False)
            if p.returncode == 0:
                return name
            if run(['docker', 'inspect', name, '--format', '{{.State.Running}}']).stdout.strip() != 'true':
                raise RuntimeError(run(['docker', 'logs', name], False).stderr)
            time.sleep(.5)
        raise RuntimeError('Isolated test server did not become ready')

    def logs(name):
        p = run(['docker', 'logs', name])
        return p.stdout + p.stderr

    def host_smoke(name):
        info = json.loads(run(['docker', 'inspect', name]).stdout)[0]
        port = int(info['NetworkSettings']['Ports']['10321/tcp'][0]['HostPort'])
        assert port != 10321
        assert info['Config']['User'] == '99:100'
        status = pathlib.Path(f"/proc/{info['State']['Pid']}/status").read_text().splitlines()
        assert next(x for x in status if x.startswith('Uid:')).split()[1:] == ['99'] * 4
        assert next(x for x in status if x.startswith('Gid:')).split()[1:] == ['100'] * 4
        with socket.create_connection(('127.0.0.1', port), timeout=10) as s:
            stream = s.makefile('rb')

            def send(kind, data=None, payload=b''):
                header = {'type': kind, 'data': data or {}}
                if payload:
                    header['payload_length'] = len(payload)
                s.sendall(json.dumps(header).encode() + b'\n' + payload)

            send('describe')
            assert json.loads(stream.readline())['type'] == 'info'
            send('transcribe', {'language': 'de'})
            fmt = {'rate': 16000, 'width': 2, 'channels': 1}
            with wave.open(str(a.audio), 'rb') as audio:
                assert (audio.getframerate(), audio.getsampwidth(), audio.getnchannels()) == (16000, 2, 1)
                send('audio-start', fmt)
                while payload := audio.readframes(320):
                    send('audio-chunk', fmt, payload)
            send('audio-stop')
            event = json.loads(stream.readline())
            assert event['type'] == 'transcript', event
            transcript = event['data']['text']
            assert transcript.lower().strip().rstrip('.!?') == 'schalte das licht im wohnzimmer ein'
            return {'host_port': port, 'describe': True, 'transcript': transcript, 'uid': 99, 'gid': 100}

    try:
        before = tree(root)
        report = json.loads(cli('verify', 'custom/local').stdout)
        assert report['valid'] and report['needs_preparation'] and tree(root) == before
        listed = json.loads(cli('list').stdout)['models']
        assert any(r['model'] == 'custom/local' and r['valid'] for r in listed)
        name = start('first')
        results['first_start'] = host_smoke(name)
        assert logs(name).count('custom_model_prepared') == 1
        assert 'model_download' not in logs(name)
        assert digest(root / 'selected-128.data') == original_hashes[str(a.models / 'community-128/Kroko-DE-Community-128-L-Streaming-001.data')]
        prepared = tree(root)
        run(['docker', 'restart', name])
        deadline = time.monotonic() + 90
        while run(['docker', 'exec', name, '/usr/local/bin/wyoming-probe'], False).returncode:
            assert time.monotonic() < deadline
            time.sleep(.5)
        results['restart'] = host_smoke(name)
        assert logs(name).count('custom_model_prepared') == 1 and tree(root) == prepared
        stop(name)
        name = start('offline-readonly', offline=True, ro=True)
        assert 'custom_model_prepared' not in logs(name) and 'model_download' not in logs(name)
        # unsupportedbe runs through the same production protocol, in this isolated network namespace.
        results['offline_readonly'] = json.loads(cli('verify', 'custom/local', ro=True).stdout)
        assert results['offline_readonly']['valid']
        probe = run(['docker', 'exec', name, '/usr/local/bin/wyoming-probe',
                     '--smoke-fast', '127.0.0.1:10321', '/test/01.wav'])
        results['offline_readonly_transcript'] = [json.loads(s) for s in probe.stdout.splitlines()]
        stop(name)
        assert tree(root) == prepared
        results['community64_custom_install'] = json.loads(cli('install', 'custom/second', model='custom/second', path='selected-64.data').stdout)
        name = start('community64', model='custom/second', path='selected-64.data', ro=True)
        results['community64_custom_start'] = host_smoke(name)
        stop(name)
        assert cli('install', 'custom/bad', model='custom/bad', path='missing.data', check=False).returncode
        assert not (root / 'custom/bad').exists()
        assert cli('status', model='classic', check=False).returncode
        before = tree(root)
        assert cli('remove', 'custom/local', '--yes', check=False).returncode
        assert tree(root) == before
        # A synthetic unsupported header, not an actual unsupported model or key.
        unsupported = a.output / 'unsupported-header-only'
        unsupported.mkdir()
        os.chown(unsupported, 99, 100)
        h = json.dumps({'free': False, 'type': 'zipformer2', 'language': {'iso': 'DE'}}).encode()
        (unsupported / 'unsupported.data').write_bytes(len(h).to_bytes(4, 'little') + h)
        before = tree(unsupported)
        p = cli('install', 'custom/unsupported', base=unsupported, model='custom/unsupported', path='unsupported.data', check=False)
        assert p.returncode and 'requires an explicit boolean free=true' in p.stderr
        assert tree(unsupported) == before and len(list(unsupported.iterdir())) == 1
        p = run(['docker', 'run', '--rm', '--network', 'none',
                 *options(base=unsupported, model='custom/unsupported', path='unsupported.data'), a.image], False)
        assert p.returncode and 'requires an explicit boolean free=true' in p.stderr
        assert tree(unsupported) == before
        results['unsupported_header_rejected_without_writes'] = True
        report = json.loads(cli('status').stdout)
        decoder = root / pathlib.Path(report['directory']).relative_to('/models') / 'decoder.onnx'
        decoder.write_bytes(b'preserve corrupt test cache')
        before = tree(root)
        assert cli('install', 'custom/local', check=False).returncode and tree(root) == before
        results['invalid_cache_preserved'] = True
        assert (root / 'unrelated.keep').read_text() == 'preserve other appdata'
        assert all(digest(pathlib.Path(p)) == h for p, h in original_hashes.items())
        results['original_fixtures_unchanged'] = True
        results['all_passed'] = True
    finally:
        for name in names:
            if name.startswith(prefix):
                run(['docker', 'rm', '--force', name], False)
        (a.output / 'results.json').write_text(json.dumps(results, indent=2))
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
