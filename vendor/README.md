# PhotoRec for development on Windows

Open Recovr's classic engine runs **PhotoRec 7.2** (command-line version),
© Christophe GRENIER <grenier@cgsecurity.org>, released under the GNU General Public
License v2 or later (see `testdisk-7.2/COPYING.txt` once fetched).

PhotoRec is **not included in Open Recovr's releases** and its binaries are **not stored in this
repository**. Users who want the classic engine fetch it from Settings: Open Recovr downloads the
official archive from cgsecurity.org into their own data folder (`%LOCALAPPDATA%\Open Recovr\testdisk-7.2`),
see `src/addon.rs`.

For development (running a build from the source tree, the add-on test, the benchmarks in
`scripts/bench`), fetch the same files here with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1
```

The script and the add-on both download the official archive <https://www.cgsecurity.org/testdisk-7.2.win64.zip>.
They keep only `photorec_win.exe`, the Cygwin libraries it needs (`cyg*.dll`, GPL/LGPL with the
Cygwin linking exception), its terminal description and the licence and credit files. Every kept
file is checked against a known SHA-256 (the same list in both places).

The other programs in the archive (QPhotoRec, TestDisk, fidentify) are left out.
Open Recovr starts PhotoRec as a separate program and does not modify it.

PhotoRec source code: <https://www.cgsecurity.org/wiki/TestDisk_Download> (`testdisk-7.2.tar.bz2`).
