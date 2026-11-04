#!/usr/bin/env python3
"""Two-process Linux runtime baseline. Requires Sway, grim and cryptography.
Uses an isolated display and test-only publisher trust, never uploads anything.
"""
import argparse, csv, hashlib, json, os, pathlib, signal, subprocess, tempfile, time
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

p = argparse.ArgumentParser()
p.add_argument('--binary', required=True)
p.add_argument('--bundle', required=True)
p.add_argument('--output', required=True)
p.add_argument('--seconds', type=int, default=30)
a = p.parse_args()
if a.seconds < 11: p.error("--seconds must be at least 11 for settled samples")
out = pathlib.Path(a.output).resolve(); out.mkdir(parents=True, exist_ok=True)
binary = pathlib.Path(a.binary).resolve(); bundle = pathlib.Path(a.bundle).resolve()
env = os.environ.copy()
for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'SWAYSOCK'): env.pop(key, None)
runtime = pathlib.Path(tempfile.mkdtemp(prefix='studio-baseline-'))
env.update(XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS='headless', WLR_RENDERER='pixman', WLR_LIBINPUT_NO_DEVICES='1', LIBGL_ALWAYS_SOFTWARE='1')
config = out / 'sway.conf'
config.write_text('xwayland disable\noutput HEADLESS-1 mode 2880x900 scale 1\noutput * bg #23132e solid_color\ndefault_border none\ndefault_floating_border none\nseat seat0 fallback true\n')
now = int(time.time())
public = Ed25519PrivateKey.from_private_bytes(bytes([7])*32).public_key().public_bytes(Encoding.Raw, PublicFormat.Raw).hex()
trust = runtime / 'test-trust.json'
trust.write_text(json.dumps(dict(schemaVersion=1,snapshotId='countdown-benchmark-only',version=1,validFrom=now-60,expiresAt=now+86400,keys=[dict(publisherId='studio-templates',keyId='template-key-1',publicKey=public,validFrom=now-60,expiresAt=now+86400,enabled=True)],revocations=[])))
env['STUDIO_TRUST_STORE'] = str(trust)
compositor = None; app = None
meta = dict(binary=str(binary), bundle=str(bundle), binary_bytes=binary.stat().st_size, bundle_bytes=bundle.stat().st_size, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), bundle_sha256=hashlib.sha256(bundle.read_bytes()).hexdigest(), viewport_per_app=[1440,900], output_size=[2880,900], instances=2, compositor_renderer="pixman", software_gl_requested=True, vulkan_adapter="not pinned", startup_runs=[], started_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()))
def save(): (out/'results.json').write_text(json.dumps(meta,indent=2)+'\n')
def shot(path):
    subprocess.run(['grim',str(path)],env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
def metrics(pid):
    base=pathlib.Path('/proc')/str(pid)
    stat=(base/'stat').read_text().rsplit(')',1)[1].split()
    status={line.split(':')[0]:line.split(':',1)[1].strip() for line in (base/'status').read_text().splitlines() if ':' in line}
    smaps={line.split(':')[0]:line.split(':',1)[1].strip() for line in (base/'smaps_rollup').read_text().splitlines() if ':' in line}
    return dict(cpu_seconds=(int(stat[11])+int(stat[12]))/os.sysconf('SC_CLK_TCK'),rss_kib=int(status['VmRSS'].split()[0]),peak_rss_kib=int(status['VmHWM'].split()[0]),pss_kib=int(smaps['Pss'].split()[0]),threads=int(status['Threads']))

apps=[]
def tree_pids(node):
    result=[node['pid']] if node.get('pid') else []
    for child in node.get('nodes',[])+node.get('floating_nodes',[]): result+=tree_pids(child)
    return result
try:
    compositor=subprocess.Popen(['dbus-run-session','--','sway','-c',str(config)],env=env,stdout=open(out/'sway.log','w'),stderr=subprocess.STDOUT,start_new_session=True)
    for _ in range(200):
        sockets=[x for x in runtime.glob('wayland-*') if x.is_socket()]
        ipc=list(runtime.glob('sway-ipc.*.sock'))
        if sockets and ipc: break
        if compositor.poll() is not None: raise RuntimeError('Sway failed')
        time.sleep(.05)
    else: raise RuntimeError('No compositor socket')
    env['WAYLAND_DISPLAY']=sockets[0].name;env['SWAYSOCK']=str(ipc[0])
    start=time.perf_counter()
    for index in range(2):
        apps.append(subprocess.Popen([str(binary),'--bundle',str(bundle)],env=env,stdout=open(out/f'app-{index}.log','w'),stderr=subprocess.STDOUT))
    for _ in range(300):
        if any(p.poll() is not None for p in apps): raise RuntimeError('App exited')
        tree=json.loads(subprocess.check_output(['swaymsg','-s',env['SWAYSOCK'],'-t','get_tree','-r'],env=env))
        if all(p.pid in tree_pids(tree) for p in apps):break
        time.sleep(.05)
    else:raise RuntimeError('Two app windows did not appear')
    (out/'window-tree.json').write_text(json.dumps(tree,indent=2))
    meta['pids']=[p.pid for p in apps];meta['both_windows_mapped_seconds']=time.perf_counter()-start
    rows=[]
    with (out/'samples.csv').open('w',newline='') as f:
        writer=csv.DictWriter(f,fieldnames=['elapsed_seconds','instance','cpu_seconds','rss_kib','peak_rss_kib','pss_kib','threads']);writer.writeheader()
        for second in range(a.seconds+1):
            for index,process in enumerate(apps):
                if process.poll() is not None: raise RuntimeError('App exited while sampling')
                row=dict(elapsed_seconds=time.perf_counter()-start,instance=index,**metrics(process.pid))
                rows.append(row);writer.writerow(row)
            f.flush()
            if second in (0,5,a.seconds):shot(out/f'frame-{second:02}.png')
            if second<a.seconds:time.sleep(max(0,start+second+1-time.perf_counter()))
    import statistics
    settled=[r for r in rows if r['elapsed_seconds']>=10]
    summaries=[]
    for index in range(2):
        group=[r for r in settled if r['instance']==index]
        summaries.append(dict(instance=index,rss_mib_median=statistics.median(r['rss_kib'] for r in group)/1024,pss_mib_median=statistics.median(r['pss_kib'] for r in group)/1024,peak_rss_mib=max(r['peak_rss_kib'] for r in rows if r['instance']==index)/1024))
    meta['settled_10_to_30_seconds']=summaries
    meta['combined_rss_mib_median']=statistics.median((rows[i]['rss_kib']+rows[i+1]['rss_kib'])/1024 for i in range(0,len(rows),2) if rows[i]['elapsed_seconds']>=10)
    meta['combined_pss_mib_median']=statistics.median((rows[i]['pss_kib']+rows[i+1]['pss_kib'])/1024 for i in range(0,len(rows),2) if rows[i]['elapsed_seconds']>=10)
    meta['completed']=True;save();print(json.dumps(meta,indent=2))
except BaseException as exc:
    meta['error']=repr(exc);save();raise
finally:
    for process in apps:
        if process.poll() is None:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()
    if compositor and compositor.poll() is None:
        os.killpg(compositor.pid,signal.SIGTERM)
        try:compositor.wait(timeout=5)
        except subprocess.TimeoutExpired:os.killpg(compositor.pid,signal.SIGKILL)
