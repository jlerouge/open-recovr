# PhotoRec for Windows

Open Recovr's full analysis on Windows is done by **PhotoRec 7.2** (command-line version),
© Christophe GRENIER <grenier@cgsecurity.org>, released under the GNU General Public
License v2 or later (see `testdisk-7.2/COPYING.txt` once fetched).

The binaries are **not stored in this repository**. Fetch them with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1
```

The script downloads the official archive <https://www.cgsecurity.org/testdisk-7.2.win64.zip>,
keeps only `photorec_win.exe`, the Cygwin libraries it needs (`cyg*.dll`, GPL/LGPL with the
Cygwin linking exception) and the licence and credit files. It checks every kept file against a
known SHA-256 before putting it in `vendor/testdisk-7.2/`.

The other programs in the archive (QPhotoRec, TestDisk, fidentify) are left out.
Open Recovr starts PhotoRec as a separate program and does not modify it.

PhotoRec source code: <https://www.cgsecurity.org/wiki/TestDisk_Download>
(`testdisk-7.2.tar.bz2`). The release workflow attaches it to every GitHub release.
