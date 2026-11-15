#!/usr/bin/env python3
"""macOS hot reload integration check using opt-in native callback replay.

Build zant-cli first with scripts/cargo.sh. Run hot-check.py [new evidence dir].
Keeps the fresh create app, screenshots, state envelopes, PIDs and logs. This
checks the host/runtime boundary; scripts/mac/ui-scenario.swift separately
checks real desktop input when WindowServer access is available.
"""
import os, json, re, signal, subprocess, time, pathlib, shutil
R=pathlib.Path(__file__).resolve().parents[2]
import sys, tempfile
D=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else pathlib.Path(tempfile.mkdtemp(prefix='zant-hot-check-'))
D.mkdir(parents=True,exist_ok=True)
print(f'Evidence: {D}',flush=True)
cli=R/'target/debug/zant'
subprocess.run([str(cli),'create','app','--yes','--sandbox','false','--sign','none'],cwd=D,check=True,stdout=(D/'create.log').open('w'),stderr=subprocess.STDOUT)
a=D/'app'; p=a/'src/App.tsx'
s=p.read_text().replace('Text }','Text, Column }').replace('const theme = useTheme();','const [mode, setMode] = useState(7);\n  const theme = useTheme();').replace('<Input value=', '<Input key="name" value=').replace('<Text color=', '<Text key="count" color=')
s=s.replace('      <Dashboard', '      <Text key="mode">Mode: {mode}</Text>\n      <Dashboard')
rows=''.join(f'<Paragraph key="row-{i}">Scrollable row {i}</Paragraph>' for i in range(30))
s=s.replace('    </View>',f'      <Column key="scroller" height={{100}} overflow="scroll">{rows}</Column>\n    </View>')
p.write_text(s)
c=a/'src/components/CounterCard.tsx';c.write_text(c.read_text().replace('<Button variant=', '<Button key="inc" variant='))
fifo=D/'control';os.mkfifo(fifo)
env=os.environ|{'CARGO_NET_OFFLINE':'true','CARGO_HOME':str(R/'.cargo-home'),'ZANT_CARGO':str(R/'scripts/cargo.sh'),'ZANT_HOT_POLL':'1','ZANT_HOT_TRACE':'1','ZANT_HOT_SNAPSHOT':str(D/'shots'),'ZANT_LAYOUT_FILE':str(D/'layout'),'ZANT_HOT_CONTROL':str(fifo),'ZANT_FRAME_STATS':'1'}
log=(D/'run.log').open('w');proc=subprocess.Popen([str(cli),'run'],cwd=a,env=env,stdout=log,stderr=subprocess.STDOUT)
(D/'cli.pid').write_text(str(proc.pid))

def output():return (D/'run.log').read_text()
def wait(fn,desc,timeout=60):
 end=time.monotonic()+timeout
 while time.monotonic()<end:
  if fn():return
  if proc.poll() is not None:raise RuntimeError(f'exited {proc.returncode}: '+output()[-2000:])
  time.sleep(.04)
 raise TimeoutError(desc+'\n'+output()[-2000:])

def shots():return len(list((D/'shots').glob('reload-*.png')))
def reload(edit,label):
 n=shots();start=time.time();edit();wait(lambda:shots()>n,label)
 (D/'events.jsonl').open('a').write(json.dumps({'label':label,'wall':start,'complete':time.time(),'pid':re.findall(r'host_pid=(\d+)',output())[-1]})+'\n')
 time.sleep(.15)

def send(text): control.write(text+'\n');control.flush();time.sleep(.09)
def bounds(key):
 for line in (D/'layout').read_text().splitlines():
  f=line.split('\t')
  if f[0]==key:
   x,y,w,h=map(float,f[1:]);return x+w/2,y+h/2
 raise ValueError(key)
def click(key):
 x,y=bounds(key);send(f'pointer 2 {x} {y}');send(f'pointer 3 {x} {y}')
def state():
 n=shots();send('snapshot');wait(lambda:shots()>n,'snapshot')
 # Force a real reload so save() serializes the running state.
 reload(lambda:p.write_text(p.read_text()+'\n'), 'state-save')
 paths=sorted((D/'shots').glob('state-*.json'));return json.loads(paths[-1].read_text())

try:
 wait(lambda:shots()>0,'initial',120)
 # A generation must share the framework/std, with no template TLS of its own.
 ui=a/'target/zant-hot/gen/libui-1.dylib'
 load_commands=subprocess.check_output(['otool','-l',str(ui)],text=True)
 assert '__thread' not in load_commands,'UI generation contains thread-local storage'
 links=subprocess.check_output(['otool','-L',str(ui)],text=True)
 assert 'libzant_dylib.dylib' in links and '@rpath/libstd-' in links,links
 framework=a/'target/zant-hot/target/debug/deps/libzant_dylib.dylib'
 def framework_stamp():
  st=framework.stat();return st.st_ino,st.st_size,st.st_mtime_ns
 framework_before=framework_stamp()
 control=fifo.open('w')
 for i in range(3):click('inc')
 click('name');send('key 97 8');send('text Keep me');send('key 4353 0');send('key 4353 0')
 x,y=bounds('scroller');send(f'scroll {x} {y} -160')
 before=state();(D/'before.json').write_text(json.dumps(before,indent=2))
 assert before['app']['clicks']==3,before
 assert before['app']['name']=='Keep me',before
 reload(lambda:p.write_text(p.read_text().replace('修改 App.tsx 和 components/ 试试。','Reloaded text in the same window.')),'text')
 after=state();(D/'after.json').write_text(json.dumps(after,indent=2));assert before==after,(before,after)
 reload(lambda:p.write_text(p.read_text().replace('gap={14}', 'gap={16}')),'layout')
 def component():
  (a/'src/components/Added.tsx').write_text('import { Text } from "zant"; export function Added() { return <Text key="added">New component loaded</Text>; }')
  p.write_text('import { Added } from "./components/Added";\n'+p.read_text().replace('      <Text key="mode">','      <Added />\n      <Text key="mode">'))
 reload(component,'new-component')
 reload(lambda:p.write_text(p.read_text().replace('useState(7)', 'useState(true)')),'changed-type')
 typed=state();(D/'typed.json').write_text(json.dumps(typed,indent=2));assert typed['app']['mode'] is True and typed['app']['clicks']==3 and typed['app']['name']=='Keep me',typed
 good=p.read_text();p.write_text(good+'\nexport default broken !!!\n');wait(lambda:'编译失败，界面保持上一版本' in output(),'TSX error');send('snapshot');time.sleep(.2)
 reload(lambda:p.write_text(good),'fixed-tsx')
 native=a/'native/system.rs';ns=native.read_text();native.write_text(ns+'\npub const BROKEN: = ;\n');old=output().count('编译失败，界面保持上一版本');wait(lambda:output().count('编译失败，界面保持上一版本')>old,'Rust error');send('snapshot');time.sleep(.2)
 reload(lambda:native.write_text(ns.replace('Str::copy(version)','Str::copy("native reloaded")')),'fixed-native')
 assert framework_stamp()==framework_before,'framework rebuilt during app edits'
 count=output().count('检测到改动，重新编译');p.write_text(p.read_text());time.sleep(1);assert output().count('检测到改动，重新编译')==count,'unchanged save rebuilt'
 # Save twice after the build has started: exactly one follow-up build.
 count=output().count('检测到改动，重新编译');p.write_text(p.read_text().replace('Reloaded text','Burst first'))
 wait(lambda:output().count('检测到改动，重新编译')>count,'burst build started');p.write_text(p.read_text().replace('Burst first','Burst second'));time.sleep(.03);p.write_text(p.read_text().replace('Burst second','Burst final'))
 wait(lambda:output().count('检测到改动，重新编译')>=count+2,'follow-up');wait(lambda:'Burst final' in ''.join(x.read_text(errors='ignore') for x in (a/'target/zant-hot/target/debug/build').glob('*/out/App.rs')),'final generated source');time.sleep(2);assert output().count('检测到改动，重新编译')==count+2,output()[-2000:]
 pid=re.findall(r'host_pid=(\d+)',output())[-1]
 cfg=a/'app.config.json';cfg.write_text(cfg.read_text().replace('"width": 680','"width": 700'))
 wait(lambda:len(re.findall(r'host_pid=(\d+)',output()))>=2,'config restart');assert re.findall(r'host_pid=(\d+)',output())[-1]!=pid
 # The control FIFO remains attached to the old host; reconnect to the new one.
 control.close();control=fifo.open('w')
 pid=re.findall(r'host_pid=(\d+)',output())[-1]
 cargo=a/'Cargo.toml';cargo.write_text(cargo.read_text().replace('version = "0.1.0"','version = "0.1.1"'))
 wait(lambda:len(re.findall(r'host_pid=(\d+)',output()))>=3,'manifest restart');assert re.findall(r'host_pid=(\d+)',output())[-1]!=pid
 control.close();control=fifo.open('w');send('close');proc.wait(timeout=10);assert proc.returncode==0,proc.returncode
 (D/'result.txt').write_text('PASS: shared framework/std, no UI TLS, no framework rebuild, callback replay, state/view preservation, errors, native, unchanged saves, burst, restarts, close\n')
except Exception as e:
 (D/'result.txt').write_text(repr(e)+'\n');raise
finally:
 if proc.poll() is None:
  proc.send_signal(signal.SIGINT)
  try:proc.wait(timeout=10)
  except subprocess.TimeoutExpired:proc.kill()
