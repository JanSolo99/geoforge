"""Prepare disposable native results, after building ug. Never changes source."""
from pathlib import Path
import shutil,subprocess
root=Path('/tmp/unigis-globalids-native');root.mkdir(exist_ok=False)
gdb=root/'output.gdb'
shutil.copytree('testdata/geoproc/globalids_base.gdb',gdb,ignore=shutil.ignore_patterns('*.lock'))
items=['points','repeat','records','empty','collision','group']
subprocess.run(['target/debug/ug','run','add_global_ids','in_datasets='+';'.join(str(gdb/i) for i in items)],check=True)
subprocess.run(['target/debug/ug','run','add_global_ids','in_datasets='+str(gdb/'points')],check=True)
