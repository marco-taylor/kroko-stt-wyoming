#!/usr/bin/env python3
"""Isolated Docker acceptance checks. Never mounts production or publishes ports.
Requires a locally built candidate, locally licensed fixtures, and --download to
exercise the real pinned provider download (~156 MB). No benchmark files used.
"""
import argparse,hashlib,json,os,pathlib,shutil,subprocess,time,uuid

def run(cmd,ok=True):
    p=subprocess.run(cmd,text=True,capture_output=True,timeout=350)
    if ok and p.returncode:raise RuntimeError(f'{cmd[0:4]} failed: {p.stderr[-3000:]} {p.stdout[-1000:]}')
    return p

def digest(path):
    with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--image',required=True);ap.add_argument('--fixtures',type=pathlib.Path,required=True);ap.add_argument('--wav-dir',type=pathlib.Path,required=True);ap.add_argument('--output',type=pathlib.Path,required=True);ap.add_argument('--download',action='store_true');ap.add_argument('--binaries',type=pathlib.Path);a=ap.parse_args()
    a.output=a.output.resolve();a.output.mkdir(exist_ok=False);a.fixtures=a.fixtures.resolve();a.wav_dir=a.wav_dir.resolve();names=[];results={};prefix='kroko-mm-test-'+uuid.uuid4().hex[:8];components=['encoder.onnx','decoder.onnx','joiner.onnx','tokens.txt']
    def folder(name):
        p=a.output/name;p.mkdir();os.chown(p,99,100);p.chmod(0o755);return p
    def options(root,network='none',ro=False):
        overrides=[]
        if a.binaries:
            for binary in ['kroko-wyoming','model-manager','wyoming-probe']:overrides+=['--mount',f'type=bind,src={a.binaries.resolve()/binary},dst=/usr/local/bin/{binary},readonly']
        return overrides+['--network',network,'--read-only','--user','99:100','--cap-drop','ALL','--security-opt','no-new-privileges','--mount',f'type=bind,src={root},dst=/models'+(',readonly' if ro else ''),'--mount',f'type=bind,src={a.wav_dir},dst=/fixtures,readonly','--env','PORT=19321','--env','NUM_THREADS=1','--env','LOG_LEVEL=warn']
    def cli(root,*args,ok=True):
        return run(['docker','run','--rm',*options(root),'--entrypoint','/usr/local/bin/model-manager',a.image,*args],ok)
    def start(root,label,model=None,network='none',ro=False):
        name=prefix+'-'+label;names.append(name);cmd=['docker','run','-d','--name',name,*options(root,network,ro)]
        if model:cmd+=['--env','KROKO_MODEL='+model]
        cmd+=[a.image];run(cmd);return name
    def logs(name):return run(['docker','logs',name],False).stdout+run(['docker','logs',name],False).stderr
    def wait(name):
        deadline=time.monotonic()+300
        while time.monotonic()<deadline:
            p=run(['docker','exec',name,'/usr/local/bin/wyoming-probe'],False)
            if p.returncode==0:return p.stdout
            if run(['docker','inspect',name,'--format','{{.State.Running}}']).stdout.strip()!='true':raise RuntimeError('server failed: '+logs(name))
            time.sleep(1)
        raise RuntimeError('startup timed out: '+logs(name))
    def stop(name):run(['docker','stop','--time','5',name]);run(['docker','rm',name]);names.remove(name)
    def smoke(name):
        p=run(['docker','exec',name,'/usr/local/bin/wyoming-probe','--smoke-fast','127.0.0.1:19321','/fixtures/01.wav','/fixtures/03.wav']);return [json.loads(s) for s in p.stdout.splitlines()]
    def tree(root):return {str(p.relative_to(root)):digest(p) for p in root.rglob('*') if p.is_file()}
    def copy_model(root,model):
        d=root/model;d.mkdir(parents=True)
        for n in components:shutil.copyfile(a.fixtures/model/n,d/n)
        return d
    def save(): (a.output/'results.json').write_text(json.dumps(results,indent=2))
    try:
        if a.download:
            root=folder('auto');server=start(root,'download',network='bridge');wait(server);results['A_empty_default_download_start']={'ok':True,'smoke':smoke(server),'files':tree(root)};assert 'model_download' in logs(server);stop(server);save()
            before=tree(root);server=start(root,'existing-offline',network='none',ro=True);wait(server);assert 'model_download' not in logs(server);results['B_present_no_download']={'ok':True,'smoke':smoke(server)};blocked=cli(root,'remove','community-128','--yes',ok=False);assert blocked.returncode and tree(root)==before
            run(['docker','restart',server]);wait(server);assert 'model_download' not in logs(server);results['C_restart_no_download']={'ok':True};stop(server);assert tree(root)==before;save()
            # The shipped v0.2.0 installer wrote no language in installation.json.
            legacy = folder('v020-community-record')
            source = root / 'de/community-128'
            if not source.exists():source = root / 'community-128'
            shutil.copytree(source, legacy/'community-128')
            record_path = legacy/'community-128/installation.json'
            record = json.loads(record_path.read_text());record.pop('language', None)
            record_path.write_text(json.dumps(record))
            manifest_path = legacy/'community-128/model.json'
            manifest = json.loads(manifest_path.read_text())
            manifest.pop('feature_dim', None);manifest.pop('stream_language', None)
            manifest_path.write_text(json.dumps(manifest))
            before = tree(legacy)
            server=start(legacy,'v020-community-upgrade',model='community-128',ro=True)
            wait(server);assert 'model_download' not in logs(server)
            results['v020_community_record_without_language']={'ok':True,'smoke':smoke(server)}
            stop(server);assert tree(legacy)==before;save()
        else:results['A_B_C']={'skipped':'Use --download to explicitly authorize the pinned provider download'}
        if a.download:
            root=folder('killed-download');server=start(root,'interrupted-live',network='bridge');deadline=time.monotonic()+15
            while not list(root.rglob('.install-*')) and time.monotonic()<deadline:time.sleep(.01)
            assert list(root.rglob('.install-*')), 'download did not enter staging'
            run(['docker','kill',server]);run(['docker','rm',server]);names.remove(server)
            assert not (root/'de/community-128').exists();p=cli(root,'verify','community-128',ok=False);assert p.returncode and json.loads(p.stdout)['present'] is False
            results['J_live_download_killed_before_publish']={'ok':True,'status':json.loads(p.stdout)};save()
        root=folder('classic');d=copy_model(root,'classic');before=tree(root);server=start(root,'classic',model=None,ro=True);wait(server);assert 'model_download' not in logs(server);results['D_classic_existing']={'ok':True,'smoke':smoke(server)};stop(server);assert tree(root)==before;save()
        # Saved Classic selection must survive an image update, even if the new
        # default's complete model is present beside it.
        copy_model(root,'community-128');before=tree(root);server=start(root,'classic-explicit-upgrade',model='classic',ro=True);wait(server);assert 'Kroko model: de/classic' in logs(server) and 'model_download' not in logs(server);results['D_explicit_classic_upgrade_with_community_present']={'ok':True,'smoke':smoke(server)};stop(server);assert tree(root)==before;save()
        for layout in ['flat','named']:
            root=folder('legacy-'+layout);d=root if layout=='flat' else root/'sherpa-onnx-streaming-zipformer-de-kroko-2025-08-06'
            if d!=root:d.mkdir()
            for n in components:shutil.copyfile(a.fixtures/'classic'/n,d/n)
            before=tree(root);server=start(root,'legacy-'+layout,ro=True);wait(server);results['D_legacy_'+layout]={'ok':True,'smoke':smoke(server)};stop(server);assert tree(root)==before;save()
        root=folder('missing');p=cli(root,'install','classic',ok=False);assert p.returncode and 'Classic is local-only' in p.stderr and not list(root.iterdir());results['E_classic_missing']={'ok':True,'error':p.stderr};save()
        root=folder('community64-existing');copy_model(root,'community-64');before=tree(root);server=start(root,'community64-existing',model='community-64',ro=True);wait(server);assert 'model_download' not in logs(server);results['D_community64_existing']={'ok':True,'smoke':smoke(server)};stop(server);assert tree(root)==before;save()
        root=folder('partial');d=root/'community-128';d.mkdir();(d/'encoder.onnx').write_bytes(b'keep existing partial');before=tree(root);p=cli(root,'install','community-128',ok=False);assert p.returncode and 'SHA256' in p.stderr and tree(root)==before;results['F_partial_preserved']={'ok':True,'error':p.stderr};save()
        root=folder('manual');d=root/'community-128';d.mkdir();package='Kroko-DE-Community-128-L-Streaming-001.data';shutil.copyfile(a.fixtures/'community-128'/package,d/package);os.chown(d,99,100);sha=digest(d/package);server=start(root,'manual',model='community-128');wait(server);assert 'model_download' not in logs(server);assert not (d/package).exists();results['G_manual_original_data']={'ok':True,'smoke':smoke(server),'status':json.loads(cli(root,'verify','community-128').stdout)};stop(server);save()
        root=folder('custom');d=root/'custom/demo';d.mkdir(parents=True)
        files=[]
        for n in components:shutil.copyfile(a.fixtures/'classic'/n,d/n);files.append({'role':n.split('.')[0],'path':n,'sha256':digest(d/n)})
        manifest={'schema_version':1,'id':'custom/demo','display_name':'Isolated manifest test','language':'de','model_type':'zipformer2','format':'onnx-transducer','backend':'sherpa-onnx-transducer','version':'fixture-only','license':'Existing local Classic fixture; no redistribution','tail_samples':23040,'files':files};(d/'model.json').write_text(json.dumps(manifest));p=cli(root,'verify','custom/demo');assert json.loads(p.stdout)['valid'];before=tree(root);server=start(root,'custom',model='custom/demo',ro=True);wait(server);results['H_custom_manifest']={'ok':True,'smoke':smoke(server)};stop(server);assert tree(root)==before;save()
        root=folder('remove');copy_model(root,'community-128');copy_model(root,'classic');(root/'wyoming-config.json').write_text('{"keep":true}');before={k:v for k,v in tree(root).items() if not k.startswith('community-128/')};protected=tree(root);p=cli(root,'remove','community-128','--yes',ok=False);assert p.returncode and tree(root)==protected;os.chown(root/'community-128',99,100);p=cli(root,'remove','community-128','--yes');assert not (root/'community-128').exists() and tree(root)==before;results['I_scoped_removal']={'ok':True};save()
        # J uses deterministic injected reader failures and SHA failures in Rust
        # unit tests. This acceptance check also rejects an interrupted local file.
        root=folder('interrupted');d=root/'community-128';d.mkdir();(d/'Kroko-DE-Community-128-L-Streaming-001.data').write_bytes(b'interrupted download');before=tree(root);p=cli(root,'verify','community-128',ok=False);assert p.returncode and tree(root)==before;results['J_interrupted_not_installed']={'ok':True,'error':p.stdout};save()
        results['complete']=True;save();print(json.dumps(results,indent=2))
    finally:
        for n in list(names):
            if n.startswith(prefix):run(['docker','rm','--force',n],False)
if __name__=='__main__':main()
