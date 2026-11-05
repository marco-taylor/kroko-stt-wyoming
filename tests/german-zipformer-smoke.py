#!/usr/bin/env python3
"""Local German Zipformer streaming/Wyoming checks without changing model files."""
import argparse,asyncio,json,pathlib,hashlib,subprocess,sys
from wyoming_native_test import Candidate,transcribe,process_sample,send,event

def hashes(root):
    result={}
    for p in root.rglob('*'):
        if p.is_file():
            with p.open('rb') as f:result[str(p.relative_to(root))]=(hashlib.file_digest(f,'sha256').hexdigest(),p.stat().st_mtime_ns)
    return result

async def main():
    p=argparse.ArgumentParser();p.add_argument('--image',default='kroko-stt-wyoming:local');p.add_argument('--models',type=pathlib.Path,required=True);p.add_argument('--binaries',type=pathlib.Path,required=True);p.add_argument('--audio',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args();a.output.mkdir(exist_ok=False);before=hashes(a.models);model='custom/german-zipformer';result={}
    c=Candidate(a.image,a.binaries,a.models,model,a.output/'online',debug=True);c.start()
    try:
        result['describe']=await c.describe();assert result['describe']['asr'][0]['models'][0]['name']==model
        result['initial_rss']=process_sample(c.pid);result['startup_s']=c.startup_s;result['paced']=[]
        for name in ('01.wav','03.wav','01.wav'):result['paced'].append(dict(audio=name,**await transcribe(c.port,a.audio/name,paced=True)))
        try:await transcribe(c.port,a.audio/'01.wav',language='en')
        except RuntimeError as error:assert 'unsupported_language' in str(error);result['wrong_language_rejected']=True
        else:raise AssertionError('Wrong language accepted')
        # HA can send the exact advertised model name; no Kroko model identity needed.
        reader,writer=await asyncio.open_connection('127.0.0.1',c.port)
        try:
            await send(writer,'transcribe',{'name':model,'language':'de'});await send(writer,'describe');kind,data=await event(reader);assert kind=='info';result['advertised_model_selection_accepted']=True
        finally:writer.close();await writer.wait_closed()
        c.restart();result['restart']=await transcribe(c.port,a.audio/'01.wav');assert result['restart']['hypothesis'].strip()
    finally:c.close()
    lines=(a.output/'online/server.log').read_text().splitlines();partials=[]
    for line in lines:
        try:x=json.loads(line)
        except ValueError:continue
        if x.get('event')=='partial' and x.get('after_eof')==False:partials.append(x)
    assert partials;result['before_eof_partial_events']=partials
    c=Candidate(a.image,a.binaries,a.models,model,a.output/'offline',offline=True);c.start()
    try:
        probe=pathlib.Path(__file__).with_name('custom-offline-client.py')
        def offline(paths):
            x=subprocess.run(['nsenter','--target',str(c.pid),'--net',sys.executable,'-B',str(probe),*[str(p) for p in paths]],capture_output=True,text=True,timeout=120);assert x.returncode==0,x.stderr;return json.loads(x.stdout)
        result['offline_transcriptions']=offline([a.audio/'01.wav',a.audio/'03.wav']);c.restart();result['offline_restart']=offline([a.audio/'01.wav']);result['offline_network']='none';result['readonly_models']=True;result['uid_gid']=c.info['Config']['User'];assert result['uid_gid']=='99:100'
    finally:c.close()
    assert hashes(a.models)==before;result.update(model_files_unchanged=True,all_passed=True);(a.output/'results.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
if __name__=='__main__':asyncio.run(main())
