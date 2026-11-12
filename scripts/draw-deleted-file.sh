#!/bin/bash
# Draws the "where do deleted files go?" illustration of the READMEs, in every
# language and in a light and a dark version:
#   docs/illustrations/deleted-file-<lang>.svg and deleted-file-<lang>-dark.svg
# Run from the repository root: bash scripts/draw-deleted-file.sh
set -e
export LC_ALL=C.UTF-8
OUT=docs/illustrations
mkdir -p "$OUT"
FONT="-apple-system, 'Segoe UI', 'Noto Sans', Helvetica, Arial, sans-serif"

theme() {
  if [ "$1" = dark ]; then
    CARD=#161b22 BORDER=#30363d TEXT=#e6edf3 MUTED=#9198a1 BLOCK=#2d333b
    PAGE=#d9ddf0 LINE=#a9b0cf VIOLET=#8b7ffb RIM=#a5b4fc ORANGE=#f0b429
    BIN=#5c6488 BINDARK=#4a5172 OKBG=#12361f OKFG=#56d364 KOBG=#4a1518 KOFG=#ff7b72
  else
    CARD=#ffffff BORDER=#d0d7de TEXT=#1f2328 MUTED=#59636e BLOCK=#e3e7f1
    PAGE=#ffffff LINE=#c3c9e8 VIOLET=#6d5ff5 RIM=#3b3486 ORANGE=#f59e0b
    BIN=#8b93b8 BINDARK=#6f789f OKBG=#dafbe1 OKFG=#1a7f37 KOBG=#ffebe9 KOFG=#cf222e
  fi
}

# lines X Y SIZE LEADING WEIGHT FILL "line 1|line 2"
lines() {
  local IFS='|' i=0 l
  printf '<text x="%s" y="%s" font-size="%s" font-weight="%s" fill="%s" text-anchor="middle">' "$1" "$2" "$3" "$5" "$6"
  for l in $7; do
    if [ $i = 0 ]; then printf '<tspan x="%s">%s</tspan>' "$1" "$l"; else printf '<tspan x="%s" dy="%s">%s</tspan>' "$1" "$4" "$l"; fi
    i=$((i + 1))
  done
  printf '</text>\n'
}

count() { local IFS='|'; set -- $1; echo $#; }

# A grid of disk blocks; the listed cells ("col,row") are drawn in COLOR.
grid() {
  local color=$1 cells=" $2 " c r fill
  for r in 0 1 2 3 4; do
    for c in 0 1 2 3 4 5; do
      fill=$BLOCK
      case "$cells" in *" $c,$r "*) fill=$color ;; esac
      printf '<rect x="%s" y="%s" width="13" height="13" rx="3" fill="%s"/>' $((-46 + c * 16)) $((-46 + r * 16)) "$fill"
    done
  done
  printf '\n'
}

PIECES="1,0 2,0 4,1 0,2 3,2 5,3 2,4"

art1() { # a page sticking out of a bin
  cat <<EOF
<g transform="rotate(-9)">
  <rect x="-22" y="-62" width="46" height="58" rx="5" fill="$PAGE" stroke="$LINE" stroke-width="2"/>
  <rect x="-14" y="-54" width="30" height="20" rx="3" fill="$VIOLET" opacity="0.25"/>
  <path d="M-12,-36 L-3,-46 L4,-39 L8,-43 L14,-36 Z" fill="$VIOLET"/>
  <circle cx="9" cy="-49" r="3" fill="$ORANGE"/>
  <rect x="-14" y="-28" width="30" height="4" rx="2" fill="$LINE"/>
</g>
<rect x="-41" y="-24" width="82" height="11" rx="5.5" fill="$BINDARK"/>
<rect x="-11" y="-31" width="22" height="9" rx="4" fill="none" stroke="$BINDARK" stroke-width="4"/>
<path d="M-35,-11 L35,-11 L29,48 Q28,55 21,55 L-21,55 Q-28,55 -29,48 Z" fill="$BIN"/>
<path d="M-13,0 L-11,44 M0,0 L0,44 M13,0 L11,44" stroke="$CARD" stroke-opacity="0.35" stroke-width="4" stroke-linecap="round"/>
EOF
}

art2() { # the disk's table of contents, one entry deleted but still readable
  cat <<EOF
<rect x="-44" y="-60" width="88" height="114" rx="7" fill="$PAGE" stroke="$LINE" stroke-width="2"/>
<rect x="-32" y="-48" width="40" height="7" rx="3.5" fill="$MUTED" opacity="0.6"/>
<rect x="-32" y="-30" width="9" height="9" rx="2" fill="$LINE"/><rect x="-18" y="-28" width="48" height="5" rx="2.5" fill="$LINE"/>
<rect x="-32" y="-14" width="9" height="9" rx="2" fill="$LINE"/><rect x="-18" y="-12" width="38" height="5" rx="2.5" fill="$LINE"/>
<rect x="-38" y="0" width="76" height="22" rx="5" fill="$VIOLET" fill-opacity="0.14" stroke="$VIOLET" stroke-width="1.5" stroke-dasharray="4 3"/>
<rect x="-32" y="6.5" width="9" height="9" rx="2" fill="$VIOLET"/>
<text x="-18" y="15" font-size="10" font-weight="600" fill="$VIOLET">photo.jpg</text>
<rect x="-32" y="30" width="9" height="9" rx="2" fill="$LINE"/><rect x="-18" y="32" width="44" height="5" rx="2.5" fill="$LINE"/>
EOF
}

art3() { # pieces of the file scattered on the disk, under the magnifying glass
  grid "$VIOLET" "$PIECES"
  cat <<EOF
<circle cx="20" cy="12" r="24" fill="$CARD" fill-opacity="0.35" stroke="$RIM" stroke-width="5"/>
<path d="M37,29 L52,44" stroke="$ORANGE" stroke-width="9" stroke-linecap="round"/>
EOF
}

art4() { # a new file has taken the space
  grid "$ORANGE" "$PIECES"
  cat <<EOF
<circle cx="44" cy="-48" r="13" fill="$KOFG"/>
<path d="M39,-53 L49,-43 M49,-53 L39,-43" stroke="#ffffff" stroke-width="3" stroke-linecap="round"/>
EOF
}

# draw LANG ALT TOP T1 D1 T2 D2 T3 D3 T4 D4 B1 B2 B3 B4 C1 C2 C3
draw() {
  local lang=$1 alt=$2 top=$3 mode file i x cx tl t d b c ok w
  for mode in light dark; do
    theme $mode
    file="$OUT/deleted-file-$lang.svg"; [ $mode = dark ] && file="$OUT/deleted-file-$lang-dark.svg"
    {
      printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 426" font-family="%s">\n' "$FONT"
      printf '<title>%s</title>\n' "$alt"
      printf '<defs><linearGradient id="odds" x1="0" x2="1"><stop offset="0" stop-color="#2da44e"/><stop offset="0.55" stop-color="#d4a72c"/><stop offset="1" stop-color="#cf222e"/></linearGradient></defs>\n'
      printf '<text x="10" y="20" font-size="14" font-weight="600" fill="%s">%s</text>\n' "$MUTED" "$top"
      printf '<rect x="10" y="30" width="972" height="8" rx="4" fill="url(#odds)"/>\n'
      printf '<path d="M980,26 L992,34 L980,42 Z" fill="#cf222e"/>\n'
      for i in 0 1 2 3; do
        x=$((10 + i * 250)); cx=$((x + 115))
        eval "t=\${$((4 + i * 2))} d=\${$((5 + i * 2))} b=\${$((12 + i))}"
        c=""; [ $i -lt 3 ] && eval "c=\${$((16 + i))}"
        printf '<rect x="%s" y="56" width="230" height="360" rx="16" fill="%s" stroke="%s" stroke-width="1.5"/>\n' $x "$CARD" "$BORDER"
        printf '<circle cx="%s" cy="82" r="14" fill="%s"/>' $((x + 26)) "$VIOLET"
        printf '<text x="%s" y="87.5" font-size="15" font-weight="700" fill="#ffffff" text-anchor="middle">%s</text>\n' $((x + 26)) $((i + 1))
        printf '<g transform="translate(%s,150)">\n' $cx; art$((i + 1)); printf '</g>\n'
        tl=$(count "$t")
        lines $cx 244 17 21 700 "$TEXT" "$t"
        lines $cx $((244 + 21 * (tl - 1) + 26)) 14 19 400 "$MUTED" "$d"
        if [ $i -lt 3 ]; then ok="$OKBG $OKFG"; b="✓ $b"; else ok="$KOBG $KOFG"; b="✗ $b"; fi
        w=$(( ${#b} * 15 / 2 + 28 ))
        set -- "$@"
        printf '<rect x="%s" y="362" width="%s" height="28" rx="14" fill="%s"/>' $((cx - w / 2)) $w "${ok% *}"
        printf '<text x="%s" y="381" font-size="13" font-weight="700" fill="%s" text-anchor="middle">%s</text>\n' $cx "${ok#* }" "$b"
        [ -n "$c" ] && printf '<text x="%s" y="406" font-size="12" fill="%s" text-anchor="middle">%s</text>\n' $cx "$MUTED" "$c"
        if [ $i -lt 3 ]; then
          printf '<path d="M%s,141 L%s,150 L%s,159" fill="none" stroke="%s" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>\n' $((x + 236)) $((x + 244)) $((x + 236)) "$MUTED"
        fi
      done
      printf '</svg>\n'
    } > "$file"
  done
}

draw en "What happens to a deleted file: 1. In the Recycle Bin, found by the quick search. 2. Bin emptied, its name is still in the disk's table of contents, found by the deep search. 3. Table entry reused, the pieces are still on the disk, found by the deep search thanks to PhotoRec. 4. Overwritten by a new file, too late." \
  "Chances of getting the file back" \
  "In the Recycle Bin" "The file is still whole,|name included." \
  "Bin emptied" "Its name and place are|still written in the|disk's table of contents." \
  "Table entry reused" "The name is lost, but the|pieces of the file are|still on the disk." \
  "Overwritten" "A new file has taken|its place: it is gone|for good." \
  "Quick search" "Deep search" "Deep search" "Too late" \
  "by Open Recovr" "by Open Recovr" "thanks to PhotoRec"

draw fr "Ce que devient un fichier supprimé : 1. Dans la corbeille, retrouvé par la recherche rapide. 2. Corbeille vidée, son nom est encore dans le sommaire du disque, retrouvé par la recherche approfondie. 3. Sommaire réutilisé, les morceaux sont encore sur le disque, retrouvés par la recherche approfondie grâce à PhotoRec. 4. Écrasé par un nouveau fichier, trop tard." \
  "Chances de retrouver le fichier" \
  "Dans la corbeille" "Le fichier est intact,|avec son nom." \
  "Corbeille vidée" "Son nom et sa place sont|encore notés dans le|sommaire du disque." \
  "Sommaire réutilisé" "Le nom est perdu, mais|les morceaux du fichier|sont encore sur le disque." \
  "Écrasé" "Un nouveau fichier a pris|sa place : il est perdu|pour de bon." \
  "Recherche rapide" "Recherche approfondie" "Recherche approfondie" "Trop tard" \
  "par Open Recovr" "par Open Recovr" "grâce à PhotoRec"

draw es "Qué pasa con un archivo eliminado: 1. En la papelera, lo encuentra la búsqueda rápida. 2. Papelera vaciada, su nombre sigue en el índice del disco, lo encuentra la búsqueda profunda. 3. Índice reutilizado, los trozos siguen en el disco, los encuentra la búsqueda profunda gracias a PhotoRec. 4. Sobrescrito por un archivo nuevo, demasiado tarde." \
  "Probabilidad de recuperar el archivo" \
  "En la papelera" "El archivo está intacto,|con su nombre." \
  "Papelera vaciada" "Su nombre y su lugar|siguen anotados en el|índice del disco." \
  "Índice reutilizado" "El nombre se perdió, pero|los trozos del archivo|siguen en el disco." \
  "Sobrescrito" "Un archivo nuevo ocupó|su lugar: se perdió|para siempre." \
  "Búsqueda rápida" "Búsqueda profunda" "Búsqueda profunda" "Demasiado tarde" \
  "con Open Recovr" "con Open Recovr" "gracias a PhotoRec"

draw de "Was mit einer gelöschten Datei passiert: 1. Im Papierkorb, gefunden von der Schnellsuche. 2. Papierkorb geleert, der Name steht noch im Inhaltsverzeichnis des Laufwerks, gefunden von der Tiefensuche. 3. Verzeichnis neu belegt, die Teile liegen noch auf dem Laufwerk, gefunden von der Tiefensuche dank PhotoRec. 4. Von einer neuen Datei überschrieben, zu spät." \
  "Chance, die Datei zurückzubekommen" \
  "Im Papierkorb" "Die Datei ist unversehrt,|samt Namen." \
  "Papierkorb geleert" "Name und Ort stehen noch|im Inhaltsverzeichnis|des Laufwerks." \
  "Verzeichnis|neu belegt" "Der Name ist weg, aber die|Teile der Datei liegen|noch auf dem Laufwerk." \
  "Überschrieben" "Eine neue Datei hat|ihren Platz belegt: Sie ist|endgültig verloren." \
  "Schnellsuche" "Tiefensuche" "Tiefensuche" "Zu spät" \
  "mit Open Recovr" "mit Open Recovr" "dank PhotoRec"

draw it "Cosa succede a un file eliminato: 1. Nel Cestino, lo trova la ricerca rapida. 2. Cestino svuotato, il nome è ancora nell'indice del disco, lo trova la ricerca approfondita. 3. Indice riutilizzato, i pezzi sono ancora sul disco, li trova la ricerca approfondita grazie a PhotoRec. 4. Sovrascritto da un nuovo file, troppo tardi." \
  "Probabilità di ritrovare il file" \
  "Nel Cestino" "Il file è intatto,|nome compreso." \
  "Cestino svuotato" "Il suo nome e il suo posto|sono ancora scritti|nell'indice del disco." \
  "Indice riutilizzato" "Il nome è perso, ma i|pezzi del file sono|ancora sul disco." \
  "Sovrascritto" "Un nuovo file ha preso|il suo posto: è perso|per sempre." \
  "Ricerca rapida" "Ricerca approfondita" "Ricerca approfondita" "Troppo tardi" \
  "con Open Recovr" "con Open Recovr" "grazie a PhotoRec"

draw pt "O que acontece com um arquivo excluído: 1. Na Lixeira, a busca rápida o encontra. 2. Lixeira esvaziada, o nome ainda está no índice do disco, a busca profunda o encontra. 3. Índice reutilizado, os pedaços ainda estão no disco, a busca profunda os encontra graças ao PhotoRec. 4. Sobrescrito por um arquivo novo, tarde demais." \
  "Chances de recuperar o arquivo" \
  "Na Lixeira" "O arquivo está intacto,|com o nome." \
  "Lixeira esvaziada" "O nome e o lugar dele|ainda estão anotados no|índice do disco." \
  "Índice reutilizado" "O nome se perdeu, mas os|pedaços do arquivo ainda|estão no disco." \
  "Sobrescrito" "Um arquivo novo tomou|o lugar dele: está|perdido de vez." \
  "Busca rápida" "Busca profunda" "Busca profunda" "Tarde demais" \
  "com o Open Recovr" "com o Open Recovr" "graças ao PhotoRec"

draw nl "Wat er met een verwijderd bestand gebeurt: 1. In de Prullenbak, gevonden door snel zoeken. 2. Prullenbak geleegd, de naam staat nog in de inhoudsopgave van de schijf, gevonden door diep zoeken. 3. Inhoudsopgave hergebruikt, de stukken staan nog op de schijf, gevonden door diep zoeken dankzij PhotoRec. 4. Overschreven door een nieuw bestand, te laat." \
  "Kans om het bestand terug te krijgen" \
  "In de Prullenbak" "Het bestand is intact,|naam inbegrepen." \
  "Prullenbak geleegd" "Naam en plek staan nog|in de inhoudsopgave|van de schijf." \
  "Inhoudsopgave|hergebruikt" "De naam is weg, maar de|stukken van het bestand|staan nog op de schijf." \
  "Overschreven" "Een nieuw bestand nam|zijn plaats in: het is|voorgoed weg." \
  "Snel zoeken" "Diep zoeken" "Diep zoeken" "Te laat" \
  "met Open Recovr" "met Open Recovr" "dankzij PhotoRec"

draw pl "Co się dzieje z usuniętym plikiem: 1. W Koszu, znajdzie go szybkie wyszukiwanie. 2. Kosz opróżniony, nazwa jest wciąż w spisie treści dysku, znajdzie go głębokie wyszukiwanie. 3. Spis użyty ponownie, kawałki są wciąż na dysku, znajdzie je głębokie wyszukiwanie dzięki PhotoRec. 4. Nadpisany przez nowy plik, za późno." \
  "Szansa na odzyskanie pliku" \
  "W Koszu" "Plik jest nienaruszony,|razem z nazwą." \
  "Kosz opróżniony" "Nazwa i miejsce pliku|są wciąż zapisane|w spisie treści dysku." \
  "Spis użyty ponownie" "Nazwa przepadła, ale|kawałki pliku są wciąż|na dysku." \
  "Nadpisany" "Nowy plik zajął jego|miejsce: przepadł|na dobre." \
  "Szybkie wyszukiwanie" "Głębokie wyszukiwanie" "Głębokie wyszukiwanie" "Za późno" \
  "przez Open Recovr" "przez Open Recovr" "dzięki PhotoRec"

draw uk "Що стається з видаленим файлом: 1. У Кошику, його знайде швидкий пошук. 2. Кошик очищено, назва ще є в змісті диска, його знайде глибокий пошук. 3. Зміст використано знову, частини ще на диску, їх знайде глибокий пошук завдяки PhotoRec. 4. Перезаписано новим файлом, запізно." \
  "Шанс повернути файл" \
  "У Кошику" "Файл цілий,|разом із назвою." \
  "Кошик очищено" "Назва й місце файлу ще|записані в змісті|диска." \
  "Зміст|використано знову" "Назву втрачено, але|частини файлу ще|лежать на диску." \
  "Перезаписано" "Новий файл зайняв|його місце: його|втрачено назавжди." \
  "Швидкий пошук" "Глибокий пошук" "Глибокий пошук" "Запізно" \
  "завдяки Open Recovr" "завдяки Open Recovr" "завдяки PhotoRec"

echo "Drawn in $OUT"
