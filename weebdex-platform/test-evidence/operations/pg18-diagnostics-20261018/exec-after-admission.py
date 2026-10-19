"""Execute only the exact approved owned postmaster after parent admission."""
import os,sys
fd=int(os.environ.pop('POSTMASTER_GATE_FD'))
try:ready=os.read(fd,1)==b'G'
finally:os.close(fd)
if not ready:os._exit(73)
assert len(sys.argv)>1
os.execv(sys.argv[1],sys.argv[1:])
