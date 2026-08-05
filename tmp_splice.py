import sys
src = "/mnt/d/Projects/volume-control/crates/volumectl/src/linux_app.rs"
tail = "/mnt/d/Projects/volume-control/tmp_tail.rs"
data = open(src, encoding="utf-8").read()
lines = data.split('\n')
# Keep lines 1..164 (0..163), i.e. through `let rx_rc = Rc::new(rx);`
head = '\n'.join(lines[:164])
t = open(tail, encoding="utf-8").read()
# ensure head ends with newline before tail
if not head.endswith('\n'):
    head += '\n'
open(src, 'w', encoding="utf-8").write(head + t)
print("spliced: %d head lines + tail" % 164)
