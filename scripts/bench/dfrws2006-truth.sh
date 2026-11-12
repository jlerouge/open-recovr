#!/usr/bin/env bash
# Truth file for the DFRWS 2006 carving challenge (github.com/dfrws/dfrws2006-challenge).
#
#   dfrws2006-truth.sh <fullfiles_dir> > dfrws-2006-challenge.tsv
#
# The layout table of file-image-layout.md names scenarios ("2c, Excel, frag 1 of 2"), not files:
# the map below gives, for each of the 32 files, the first sector of each of its pieces, in order,
# copied from that table. Every piece but the last fills whole sectors up to the next row of the
# table; the last one holds the rest of the file. The files in between (a JPEG inside a Word
# document, a text...) are files of their own and are listed too.
set -euo pipefail
files=$1
# name: start-end of each piece but the last, then the start of the last
map='
1a.html 9
2c.xls 2051-3050 3072
3a.jpg 3868
1d1.html 4436-4455 4486
1d2.html 4456-4485 4502
2d.doc 7964-8284 9474
2d.jpg 8285
3d.jpg 11619-11822 11849
3d.txt 11823
3b.jpg 12222
1b.html 27496-27606 27978
1b.jpg 27607
1c.html 28244-28245 28307
1c.txt 28246
4a.zip 28439
4b.zip 28729-29528 29896
4b.html 29529
3h1.jpg 31475-31532 31753
3h2.jpg 31533-31752 31888
2a.doc 32837
2e.doc 34288-34306 34413
2e.txt 34307
3c.jpg 36292
2b.doc 36998-37649 37727-39427 39477
3f.jpg 40638-41219 41239
3g1.jpg 41611-43433 44029
3g2.jpg 43434
4c.zip 45015-45386 45390
3e.jpg 45566-45963 46104
3e.doc 45964
3i.jpg 46910
3j.jpg 94846-95628 95630
'
echo "# DFRWS 2006 carving challenge, from file-image-layout.md; extents in bytes from the start of the image"
while read -r name pieces; do
  [[ -z $name ]] && continue
  f="$files/$name"
  size=$(stat -c %s "$f")
  left=$size
  ranges=()
  for p in $pieces; do
    if [[ $p == *-* ]]; then
      a=${p%-*}; b=${p#*-}; len=$(( (b - a + 1) * 512 ))
    else
      a=$p; len=$left
    fi
    ranges+=("$((a * 512))+$len")
    left=$((left - len))
  done
  sha=$(sha256sum "$f" | cut -c1-64)
  head=$(head -c 4096 "$f" | sha256sum | cut -c1-64)
  printf '%s\t%s\t%s\tdeleted\t%s\t%s\t%s\n' "$sha" "$head" "$size" "${#ranges[@]}" "$(IFS=,; echo "${ranges[*]}")" "$name"
done <<< "$map"
