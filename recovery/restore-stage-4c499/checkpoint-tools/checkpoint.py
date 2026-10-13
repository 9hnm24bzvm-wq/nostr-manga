#!/usr/bin/env python3
"""Reviewed-file checkpoints: AES-256 ZIP STORE, readback gate, atomic receipts.
Dependency: pyzipper==0.3.6 (PyPI). Never print a password or put it in arguments.
"""
import argparse, hashlib, json, os, re, secrets, stat, subprocess, tempfile, zlib
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
import pyzipper

PREFIX = 'manga-protocol-prototype/'
BLOCKED = {'vendor', 'node_modules', '__pycache__', 'target', '.git', '.aws', '.codex', '.env'}
BAD_SUFFIX = {'.key', '.pem', '.p12', '.sqlite', '.sqlite3', '.db', '.zip', '.7z', '.tar', '.gz'}
SECRET = re.compile(rb'-----BEGIN [^-]*(?:PRIVATE KEY|OPENSSH)|(?:ghp_|github_pat_|sk-proj-|xoxb-)[A-Za-z0-9_-]{16,}')

def sha(b): return hashlib.sha256(b).hexdigest()
def encoded(obj): return (json.dumps(obj, indent=2, sort_keys=True)+'\n').encode()
def atomic_json(path, obj):
    path = Path(path); path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix='.receipt-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as f: f.write(encoded(obj)); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp): os.unlink(tmp)

def checked_files(root, allowlist):
    root = Path(root).resolve(); names = json.loads(Path(allowlist).read_text())
    if not isinstance(names, list) or len(names) != len(set(names)): raise ValueError('Invalid or duplicate allowlist')
    out = {}
    for name in sorted(names):
        p = PurePosixPath(name)
        blocked = set(p.parts) & BLOCKED
        # Explicitly reviewed generated browser runtime dependencies only.
        if p.parts[:3] == ('browser-client', 'public', 'vendor'):
            blocked -= {'vendor'}
        if p.is_absolute() or '..' in p.parts or str(p) != name or blocked or p.suffix.lower() in BAD_SUFFIX:
            raise ValueError('Disallowed path: '+name)
        source = root / name
        if any((root.joinpath(*p.parts[:i])).is_symlink() for i in range(1,len(p.parts)+1)):
            raise ValueError('Symlink rejected: '+name)
        if not source.is_file() or not source.resolve().is_relative_to(root): raise ValueError('Missing/unsafe file: '+name)
        b = source.read_bytes()
        if SECRET.search(b): raise ValueError('Credential marker requires review; file rejected: '+name)
        out[PREFIX+name] = (b, stat.S_IMODE(source.stat().st_mode))
    if not out: raise ValueError('Empty checkpoint')
    return out

def verify(archive, password):
    with pyzipper.AESZipFile(archive) as z:
        z.setpassword(password)
        names = z.namelist()
        if len(names)!=len(set(names)): raise ValueError('Duplicate archive paths')
        for i in z.infolist():
            p=PurePosixPath(i.filename)
            if p.is_absolute() or '..' in p.parts or i.compress_type != pyzipper.ZIP_STORED or not i.flag_bits&1 or i.wz_aes_strength != 3:
                raise ValueError('Unsafe path/encryption/compression')
        manifest=json.loads(z.read('CHECKPOINT-MANIFEST.json'))
        if set(names) != set(manifest['files'])|{'CHECKPOINT-MANIFEST.json'}: raise ValueError('Manifest membership mismatch')
        for name, entry in manifest['files'].items():
            b=z.read(name)  # Checks AES authentication and any ZIP CRC.
            if sha(b)!=entry['sha256'] or len(b)!=entry['bytes'] or f'{zlib.crc32(b)&0xffffffff:08x}'!=entry['crc32']:
                raise ValueError('SHA/CRC/length mismatch: '+name)
        if z.testzip() is not None: raise ValueError('ZIP integrity failure')
    return {'sha256':sha(Path(archive).read_bytes()), 'bytes':Path(archive).stat().st_size,
            'files':len(names), 'verification':'AES-256 authentication, STORE, every member SHA-256/CRC32/length verified'}

def pack(root, allowlist, provenance, archive, password_file):
    archive=Path(archive); password_file=Path(password_file)
    if archive.exists() or password_file.exists(): raise ValueError('Refusing overwrite')
    files=checked_files(root,allowlist)
    files['RECOVERY-PROVENANCE.json']=(encoded(provenance),0o644)
    for name in ['checkpoint.py','test_checkpoint_workflow.py','README.txt']:
        p=Path(__file__).parent/name
        if p.exists(): files['checkpoint-tools/'+name]=(p.read_bytes(),0o644)
    files['checkpoint-tools/reviewed-files.json']=(Path(allowlist).read_bytes(),0o644)
    manifest={'format':1,'created_utc':datetime.now(timezone.utc).isoformat(), 'provenance':provenance,
              'files':{n:{'sha256':sha(b),'crc32':f'{zlib.crc32(b)&0xffffffff:08x}','bytes':len(b),'mode':mode} for n,(b,mode) in files.items()}}
    password=secrets.token_urlsafe(32).encode()
    archive.parent.mkdir(parents=True,exist_ok=True); password_file.parent.mkdir(parents=True,exist_ok=True)
    fd=os.open(password_file,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(fd,'wb') as f: f.write(password+b'\n'); f.flush(); os.fsync(f.fileno())
    tmp=archive.with_name(archive.name+'.partial')
    if tmp.exists(): raise ValueError('Partial archive already exists')
    with pyzipper.AESZipFile(tmp,'x',compression=pyzipper.ZIP_STORED,encryption=pyzipper.WZ_AES) as z:
        z.setpassword(password); z.setencryption(pyzipper.WZ_AES,nbits=256)
        for name,(b,mode) in files.items():
            info=z.zipinfo_cls(name); info.external_attr=(stat.S_IFREG|mode)<<16; info.compress_type=pyzipper.ZIP_STORED
            z.writestr(info,b)
        z.writestr('CHECKPOINT-MANIFEST.json',encoded(manifest))
    result=verify(tmp,password)
    os.replace(tmp,archive)
    result.update(archive=str(archive.resolve()),source_files=sum(n.startswith(PREFIX) for n in files),status='local_verified_not_uploaded')
    atomic_json(str(archive)+'.local.json',result)
    return result

def commit_verified_readback(archive, downloaded, password, url, receipt):
    a=Path(archive).read_bytes(); b=Path(downloaded).read_bytes()
    if sha(a)!=sha(b) or len(a)!=len(b): raise ValueError('Download differs; previous receipt preserved')
    result=verify(downloaded,password)
    result.update(url=url,status='uploaded_downloaded_verified',verified_utc=datetime.now(timezone.utc).isoformat())
    atomic_json(receipt,result)
    return result

def upload(archive,password_file,receipt):
    password=Path(password_file).read_bytes().strip(); verify(archive,password)
    # No automatic retry: an uncertain POST must be reconciled before another upload.
    response=subprocess.run(['curl','--fail','--silent','--show-error','--max-time','180',
        '-F','reqtype=fileupload','-F','fileToUpload=@'+str(Path(archive).resolve()),
        'https://catbox.moe/user/api.php'],capture_output=True,check=True).stdout.decode().strip()
    if not re.fullmatch(r'https://files\.catbox\.moe/[A-Za-z0-9]+\.zip',response): raise ValueError('Unexpected upload response; previous receipt preserved')
    # Retain recoverable pending URL even if download fails; never mark it successful.
    atomic_json(str(archive)+'.pending-upload.json',{'url':response,'status':'awaiting_verified_readback'})
    with tempfile.TemporaryDirectory() as td:
        downloaded=Path(td)/'readback.zip'
        subprocess.run(['curl','--fail','--silent','--show-error','--max-time','180','--output',str(downloaded),response],check=True)
        return commit_verified_readback(archive,downloaded,password,response,receipt)

def main():
    p=argparse.ArgumentParser(description=__doc__); sub=p.add_subparsers(dest='cmd',required=True)
    a=sub.add_parser('pack'); a.add_argument('--root',required=True); a.add_argument('--allowlist',required=True); a.add_argument('--provenance',required=True); a.add_argument('--archive',required=True); a.add_argument('--password-file',required=True)
    for cmd in ['verify','upload']:
        a=sub.add_parser(cmd); a.add_argument('--archive',required=True); a.add_argument('--password-file',required=True)
        if cmd=='upload':a.add_argument('--receipt',required=True)
    a=p.parse_args()
    if a.cmd=='pack':result=pack(a.root,a.allowlist,json.loads(Path(a.provenance).read_text()),a.archive,a.password_file)
    elif a.cmd=='verify':result=verify(a.archive,Path(a.password_file).read_bytes().strip())
    else:result=upload(a.archive,a.password_file,a.receipt)
    print(json.dumps(result,indent=2))
if __name__=='__main__': main()
