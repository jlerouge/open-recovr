#!/usr/bin/env bash
# Turns the NIST CFReDS "File Carving" layout tables into truth files for `open-recovr --bench-score`.
#
#   cfreds-truth.sh <dir>
#
# <dir> holds the decompressed images (L0_Graphic.dd...) and layouts.tsv, made from
# https://cfreds-archive.nist.gov/FileCarving/ImageLayouts.htm, one row per piece:
#   image  file  size  start_sector  end_sector
# Pieces are named "name (n)" (or "(n,m)" for pieces kept together); a piece number seen twice
# starts a second copy of the file. The original files come from the L0 image of each family, where
# every file is whole and contiguous; a file found only in other images (nested ones) is taken
# from the first image where it is in one piece. Writes <image>.tsv next to each image.
set -euo pipefail
dir=$1
cd "$dir"
mkdir -p originals

# normalise: a typo in the tables ("000_021.png"), "name(1)" without a space
rows=$(sed -e 's/000_021\.png/000_0021.png/' -e 's/\([^ ]\)(\([0-9]\)/\1 (\2/' layouts.tsv | grep -v $'\tFill\t')

# originals: every row without a piece number is a whole file, contiguous in its image
while IFS=$'\t' read -r img file size start end; do
  [[ $file == *"("* ]] && continue
  [[ -f "originals/$file" ]] && continue
  [[ -f "$img" ]] || continue
  dd if="$img" of="originals/$file" bs=512 skip="$start" count="$size" iflag=count_bytes status=none
done <<< "$rows"

for img in $(cut -f1 <<< "$rows" | sort -u); do
  [[ -f "$img" ]] || continue
  out="${img%.dd}.tsv"
  echo "# $img, from the CFReDS layout tables; extents in bytes from the start of the image" > "$out"
  # group the pieces of each copy of each file, in piece order
  awk -F'\t' -v img="$img" '
    $1 == img {
      name = $2; n = 0
      if (match(name, / \(([0-9]+)(,[0-9]+)*\)$/)) { n = substr(name, RSTART + 2) + 0; name = substr(name, 1, RSTART - 1) }
      copy = 0
      while ((name, copy, n) in seen) copy++
      seen[name, copy, n] = 1
      key = name SUBSEP copy
      if (!(key in order)) { order[key] = ++k; keys[k] = key }
      ext[key] = ext[key] (ext[key] == "" ? "" : ";") sprintf("%06d %d+%d", n, $4 * 512, $3)
    }
    END {
      for (i = 1; i <= k; i++) { split(keys[i], p, SUBSEP); print p[1] "\t" ext[keys[i]] }
    }' <<< "$rows" | while IFS=$'\t' read -r name pieces; do
    orig="originals/$name"
    [[ -f "$orig" ]] || { echo "no original for $name in $img" >&2; continue; }
    ranges=$(tr ';' '\n' <<< "$pieces" | sort | cut -d' ' -f2 | paste -sd,)
    count=$(tr ';' '\n' <<< "$pieces" | wc -l)
    size=$(stat -c %s "$orig")
    sha=$(sha256sum "$orig" | cut -c1-64)
    head=$(head -c 4096 "$orig" | sha256sum | cut -c1-64)
    printf '%s\t%s\t%s\tdeleted\t%s\t%s\t%s\n' "$sha" "$head" "$size" "$count" "$ranges" "$name" >> "$out"
  done
  echo "$out: $(grep -vc '^#' "$out") files"
done
