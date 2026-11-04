#!/usr/bin/env python3
"""Summarize per-second Linux runtime samples without third-party packages."""
import argparse,csv,json,pathlib,statistics
p=argparse.ArgumentParser();p.add_argument('directory');a=p.parse_args()
root=pathlib.Path(a.directory)
rows=[{k:float(v) for k,v in x.items()} for x in csv.DictReader((root/'samples.csv').open())]
summary={}
for name,start,end in [('settled_countdown',10,590),('early_countdown',10,60),('late_countdown',540,590),('completion',590,605),('live_idle',605,616)]:
    group=[r for r in rows if start<=r['elapsed_seconds']<end]
    if not group:continue
    first,last=group[0],group[-1]
    seconds=last['elapsed_seconds']-first['elapsed_seconds']
    summary[name]={'samples':len(group),'cpu_percent_one_core':100*(last['cpu_seconds']-first['cpu_seconds'])/seconds if seconds else None,'rss_mib_median':statistics.median(r['rss_kib'] for r in group)/1024,'pss_mib_median':statistics.median(r['pss_kib'] for r in group)/1024,'rss_mib_max':max(r['rss_kib'] for r in group)/1024}
summary['whole_run']={'duration_seconds':rows[-1]['elapsed_seconds'],'peak_rss_mib':max(r['peak_rss_kib'] for r in rows)/1024,'peak_pss_mib':max(r['pss_kib'] for r in rows)/1024,'threads_max':max(r['threads'] for r in rows)}
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
