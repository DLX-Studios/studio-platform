#!/usr/bin/env python3
"""Linux release-runtime baseline. Requires Sway, grim, Pillow and cryptography.
Uses an isolated display and test-only publisher trust, never uploads anything.
"""
import argparse, csv, hashlib, json, os, pathlib, signal, subprocess, tempfile, time
from PIL import Image, ImageStat, ImageChops
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

p = argparse.ArgumentParser()
p.add_argument('--binary', required=True)
p.add_argument('--bundle', required=True)
p.add_argument('--output', required=True)
p.add_argument('--seconds', type=int, default=615)
p.add_argument('--cycles', type=int, default=0)
p.add_argument('--pointer', help='Path to compiled benchmark-pointer helper')
a = p.parse_args()
if a.cycles and not a.pointer: p.error("--cycles requires --pointer")
if a.seconds < 0 or a.cycles < 0: p.error("durations and cycle counts must be nonnegative")
out = pathlib.Path(a.output).resolve(); out.mkdir(parents=True, exist_ok=True)
binary = pathlib.Path(a.binary).resolve(); bundle = pathlib.Path(a.bundle).resolve()
env = os.environ.copy()
for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET', 'SWAYSOCK'): env.pop(key, None)
runtime = pathlib.Path(tempfile.mkdtemp(prefix='studio-baseline-'))
env.update(XDG_RUNTIME_DIR=str(runtime), WLR_BACKENDS='headless', WLR_RENDERER='pixman', WLR_LIBINPUT_NO_DEVICES='1', LIBGL_ALWAYS_SOFTWARE='1')
config = out / 'sway.conf'
config.write_text('xwayland disable\noutput HEADLESS-1 mode 1440x900 scale 1\noutput * bg #23132e solid_color\ndefault_border none\ndefault_floating_border none\nseat seat0 fallback true\n')
now = int(time.time())
public = Ed25519PrivateKey.from_private_bytes(bytes([7])*32).public_key().public_bytes(Encoding.Raw, PublicFormat.Raw).hex()
trust = runtime / 'test-trust.json'
trust.write_text(json.dumps(dict(schemaVersion=1,snapshotId='countdown-benchmark-only',version=1,validFrom=now-60,expiresAt=now+86400,keys=[dict(publisherId='studio-templates',keyId='template-key-1',publicKey=public,validFrom=now-60,expiresAt=now+86400,enabled=True)],revocations=[])))
env['STUDIO_TRUST_STORE'] = str(trust)
compositor = None; app = None
meta = dict(binary=str(binary), bundle=str(bundle), binary_bytes=binary.stat().st_size, bundle_bytes=bundle.stat().st_size, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), bundle_sha256=hashlib.sha256(bundle.read_bytes()).hexdigest(), viewport=[1440,900], compositor_renderer="pixman", software_gl_requested=True, vulkan_adapter="not pinned", startup_runs=[], started_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()))
def save(): (out/'results.json').write_text(json.dumps(meta,indent=2)+'\n')
def shot(path):
    subprocess.run(['grim',str(path)],env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
def metrics(pid):
    base=pathlib.Path('/proc')/str(pid)
    stat=(base/'stat').read_text().rsplit(')',1)[1].split()
    status={line.split(':')[0]:line.split(':',1)[1].strip() for line in (base/'status').read_text().splitlines() if ':' in line}
    smaps={line.split(':')[0]:line.split(':',1)[1].strip() for line in (base/'smaps_rollup').read_text().splitlines() if ':' in line}
    return dict(cpu_seconds=(int(stat[11])+int(stat[12]))/os.sysconf('SC_CLK_TCK'),rss_kib=int(status['VmRSS'].split()[0]),peak_rss_kib=int(status['VmHWM'].split()[0]),pss_kib=int(smaps['Pss'].split()[0]),threads=int(status['Threads']))
def stop_app():
    global app
    if app and app.poll() is None:
        app.terminate()
        try: app.wait(timeout=5)
        except subprocess.TimeoutExpired: app.kill(); app.wait()
    app=None
try:
    compositor=subprocess.Popen(['dbus-run-session','--','sway','-c',str(config)],env=env,stdout=open(out/'sway.log','w'),stderr=subprocess.STDOUT,start_new_session=True)
    for _ in range(200):
        sockets=[x for x in runtime.glob('wayland-*') if x.is_socket()]
        ipc=list(runtime.glob('sway-ipc.*.sock'))
        if sockets and ipc: break
        if compositor.poll() is not None: raise RuntimeError('Sway failed; see sway.log')
        time.sleep(.05)
    else: raise RuntimeError('No compositor socket')
    env['WAYLAND_DISPLAY']=sockets[0].name; env['SWAYSOCK']=str(ipc[0])
    (out/'session.json').write_text(json.dumps(dict(runtime=str(runtime),env={k:env[k] for k in ('XDG_RUNTIME_DIR','WAYLAND_DISPLAY','SWAYSOCK')},compositor_pid=compositor.pid)))
    for run in range(3):
        time.sleep(.3)
        start=time.perf_counter()
        app=subprocess.Popen([str(binary),'--bundle',str(bundle)],env=env,stdout=open(out/f'app-{run}.log','w'),stderr=subprocess.STDOUT)
        visible=None
        for _ in range(300):
            if app.poll() is not None: raise RuntimeError(f'Runtime exited {app.returncode}; see app-{run}.log')
            shot(out/'probe.png')
            # A textured image distinguishes painted app content from solid compositor background.
            with Image.open(out/'probe.png') as im: spread=sum(ImageStat.Stat(im.convert('RGB').resize((72,45))).stddev)
            if spread>30:
                visible=time.perf_counter()-start; break
            time.sleep(.02)
        if visible is None: raise RuntimeError('No visible app content')
        meta['startup_runs'].append(dict(run=run,observed_visible_seconds=visible,cache='uncontrolled first launch' if run==0 else 'warm',pid=app.pid))
        save(); print('visible',run,visible,flush=True)
        shot(out/f'startup-{run}.png')
        if run<2: stop_app(); continue
        start_sample=time.perf_counter(); previous=metrics(app.pid); previous_t=start_sample
        with open(out/'samples.csv','w',newline='') as f:
            writer=csv.DictWriter(f,fieldnames=['elapsed_seconds','cpu_percent_one_core','cpu_seconds','rss_kib','peak_rss_kib','pss_kib','threads']);writer.writeheader()
            for second in range(a.seconds+1):
                if app.poll() is not None: raise RuntimeError('Runtime exited during measurement')
                current=metrics(app.pid); moment=time.perf_counter()
                current['cpu_percent_one_core']=100*(current['cpu_seconds']-previous['cpu_seconds'])/max(moment-previous_t,.001)
                current['elapsed_seconds']=moment-start_sample
                writer.writerow(current); f.flush(); previous=current;previous_t=moment
                if second in (0,5,30,60,120,300,590,599,600,601,605,a.seconds): shot(out/f'frame-{second:04}.png')
                if second%30==0: print('sample',second,current,flush=True)
                time.sleep(max(0,start_sample+second+1-time.perf_counter()))
        if a.cycles:
            def click(x,y):
                subprocess.run([a.pointer,str(x),str(y)],env=env,check=True,stdout=subprocess.DEVNULL)
            meta['interaction_cycles']=[]
            for cycle in range(a.cycles):
                click(310,64); time.sleep(.25)
                click(190,761); time.sleep(.25)
                shot(out/'before-route.png')
                with Image.open(out/'before-route.png') as im: before=im.convert('RGB').resize((144,90))
                tick=time.perf_counter();click(280,761)
                for _ in range(100):
                    shot(out/'after-route.png')
                    with Image.open(out/'after-route.png') as im: after=im.convert('RGB').resize((144,90))
                    change=sum(ImageStat.Stat(ImageChops.difference(before,after)).mean)
                    if change>10: break
                    time.sleep(.02)
                else: raise RuntimeError('Navigation produced no substantial visual change')
                elapsed=time.perf_counter()-tick
                shot(out/f'cycle-{cycle:02}-live.png')
                click(310,64);time.sleep(.25)
                click(190,761);time.sleep(.25)
                current=metrics(app.pid)
                meta['interaction_cycles'].append(dict(cycle=cycle,observed_route_change_seconds=elapsed,**current));save()
            shot(out/'after-cycles.png')
        meta['completed']=True;save();stop_app()
except BaseException as exc:
    meta['error']=repr(exc);save();raise
finally:
    stop_app()
    if compositor and compositor.poll() is None:
        os.killpg(compositor.pid,signal.SIGTERM)
        try: compositor.wait(timeout=5)
        except subprocess.TimeoutExpired: os.killpg(compositor.pid,signal.SIGKILL)
