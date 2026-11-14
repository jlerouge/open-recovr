<div align="center">

<img src="assets/icon.svg" alt="" width="96" height="96">

# Open Recovr

**Retrouvez vos fichiers effacés, simplement.**

Un seul écran, pas d'assistant : on choisit le disque, on lance la recherche, on coche ce qu'on veut récupérer.<br>
Libre et gratuit, avec son propre moteur de récupération et [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec) en option.

[English](README.md) · **Français** · [Español](README.es.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [Português (Brasil)](README.pt.md) · [Nederlands](README.nl.md) · [Polski](README.pl.md) · [Українська](README.uk.md)

[![Tests](https://img.shields.io/github/actions/workflow/status/jlerouge/open-recovr/ci.yml?branch=main&label=tests&logo=github)](https://github.com/jlerouge/open-recovr/actions/workflows/ci.yml)
[![Version](https://img.shields.io/github/v/release/jlerouge/open-recovr?include_prereleases&sort=semver&label=version)](https://github.com/jlerouge/open-recovr/releases/latest)
[![Téléchargements](https://img.shields.io/github/downloads/jlerouge/open-recovr/total?label=t%C3%A9l%C3%A9chargements)](https://github.com/jlerouge/open-recovr/releases)
[![Licence MIT](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Systèmes](https://img.shields.io/badge/syst%C3%A8me-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)](#systèmes-pris-en-charge)
[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

<img src="docs/screenshots/fr-4-results.png" alt="Open Recovr affichant les fichiers effacés retrouvés sur le disque C:, avec l'aperçu d'une photo" width="820">

</div>

## Pourquoi Open Recovr ?

Perdre le dossier des vacances ou un mémoire, c'est déjà assez stressant. Les logiciels de récupération sont souvent chers ou pleins de jargon. Les gratuits comme PhotoRec sont excellents, mais s'utilisent dans un terminal en mode texte.

Open Recovr réunit tout sur un seul écran simple. Une analyse rapide rend les fichiers **avec leur nom et leur dossier d'origine**. Pour le reste, son propre moteur lit tout le disque et **recolle les photos, archives et documents Office fragmentés**. PhotoRec, le moteur libre qui a fait ses preuves, reste disponible comme moteur classique. La comparaison des deux est mesurée dans les [bancs d'essai](docs/benchmarks.md) (en anglais).

## Comment ça marche

| 1. Rapide ou approfondie | 2. Choisir le disque | 3. Rechercher |
|:---:|:---:|:---:|
| [![](docs/screenshots/fr-1-start.png)](docs/screenshots/fr-1-start.png) | [![](docs/screenshots/fr-2-home.png)](docs/screenshots/fr-2-home.png) | [![](docs/screenshots/fr-3-scanning.png)](docs/screenshots/fr-3-scanning.png) |
| Quelques secondes dans la corbeille, ou tout le disque. | Disque dur, SSD, carte mémoire, clé USB, CD… | Les fichiers s'affichent au fur et à mesure, avec le temps restant. |

| 4. Parcourir et prévisualiser | 5. Cocher | 6. Récupérer |
|:---:|:---:|:---:|
| [![](docs/screenshots/fr-4-results.png)](docs/screenshots/fr-4-results.png) | [![](docs/screenshots/fr-5-selected.png)](docs/screenshots/fr-5-selected.png) | [![](docs/screenshots/fr-6-recovered.png)](docs/screenshots/fr-6-recovered.png) |
| Filtres par type, format, taille ou nom. | Chaque fichier affiche ses chances de récupération. | Copie vers le disque de votre choix. |

| Paramètres | Thème sombre | Mode expert |
|:---:|:---:|:---:|
| [![](docs/screenshots/fr-8-settings.png)](docs/screenshots/fr-8-settings.png) | [![](docs/screenshots/fr-9-dark.png)](docs/screenshots/fr-9-dark.png) | [![](docs/screenshots/fr-7-expert.png)](docs/screenshots/fr-7-expert.png) |
| Langue, thème de couleurs, mises à jour, rapport. | Clair, sombre et 8 autres thèmes. | Le journal de la recherche, pour les curieux. |

Le pas-à-pas complet se trouve dans le **[guide d'utilisation](docs/user-guide.fr.md)**.

## Où vont les fichiers supprimés ?

Supprimer un fichier ne l'efface pas. Le système oublie seulement où il se trouve et marque sa place comme libre. Le contenu reste sur le disque jusqu'à ce que quelque chose de nouveau soit écrit par-dessus. Chaque recherche descend d'un cran plus bas :

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/illustrations/deleted-file-fr-dark.svg">
  <img src="docs/illustrations/deleted-file-fr.svg" alt="Ce que devient un fichier supprimé : 1. Dans la corbeille, retrouvé par la recherche rapide. 2. Corbeille vidée, son nom est encore dans le sommaire du disque, retrouvé par la recherche approfondie. 3. Sommaire réutilisé, les morceaux sont encore sur le disque, retrouvés par l'analyse complète. 4. Écrasé par un nouveau fichier, trop tard." width="100%">
</picture>

À l'étape 3, il ne reste que les octets. Le **file carving** parcourt tout le disque à la recherche des signatures qui marquent le début de chaque type de fichier, et détermine où chaque fichier se termine. Quand un fichier était rangé en plusieurs morceaux, le morceau suivant est cherché dans l'espace libre. Le moteur moderne d'Open Recovr le fait lui-même ([comment](docs/carving.md), en anglais). Le moteur classique confie ce travail à **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)**, qui reconnaît bien plus de types de fichiers. PhotoRec a posé des bases solides pour ce genre de logiciel en open source : un grand merci à Christophe Grenier, qui l'écrit et le maintient depuis plus de vingt ans.

Plus vous cherchez tôt, plus il y a à trouver. Sur un SSD, la place libérée est souvent effacée en arrière-plan peu après (TRIM) : les étapes 1 et 2 marchent encore pour les suppressions récentes, mais l'analyse complète risque de trouver peu de chose.

> [!WARNING]
> **N'enregistrez jamais les fichiers retrouvés sur le disque d'où ils viennent.** Vous pourriez écraser ceux qui attendent encore d'être récupérés. Une clé USB ou un autre disque fait très bien l'affaire. Open Recovr vous prévient si vous essayez.

> [!CAUTION]
> **Un disque qui flanche (cliquetis, grande lenteur, erreurs de lecture) peut s'abîmer un peu plus à chaque lecture.** Arrêtez de l'utiliser, et commencez par le copier en entier sur un disque sain avec un outil comme [ddrescue](https://www.gnu.org/software/ddrescue/). Ouvrez ensuite cette copie avec *Ouvrir une image disque…* et fouillez-la autant que vous voulez, sans toucher à l'original.

## Ce qu'il sait faire

- 🗑️ **Recherche rapide** dans la corbeille, en quelques secondes, sans droits particuliers.
- 🔎 **Analyse rapide (NTFS).** Lit la table des fichiers du disque (MFT). Un fichier effacé y garde son nom tant que la place n'est pas réutilisée. Les fichiers reviennent **avec leur nom et leur dossier**, en quelques minutes, même après avoir vidé la corbeille.
- 🧲 **Analyse complète.** Tout le disque est lu et les fichiers sont reconnus à leur contenu, même après un formatage ou sur une carte FAT/exFAT. Le **moteur moderne**, celui d'Open Recovr, reconnaît une quarantaine de formats (photos, RAW, vidéos, musique, Office, PDF, archives, polices) et **recolle les fichiers fragmentés**. Le **moteur classique**, PhotoRec, reconnaît **des centaines de types de fichiers** ; il se choisit dans les Paramètres.
- 💿 **Supports abîmés.** Les zones illisibles (CD/DVD rayés, cartes usées) sont sautées au lieu de tout bloquer.
- 🖼️ **Aperçu avant de récupérer.** Photos, vidéos, sons, PDF, textes et documents Word.
- 🏷️ **Des noms et des dates qui parlent.** Un fichier qui n'avait qu'un numéro reçoit un nom tiré de son contenu : `photo_2024-07-14_18-22-05.jpg` d'après la date EXIF, le titre d'un document Word ou PDF, l'artiste et le titre d'un MP3.
- ✅ **Vérifié avant de récupérer.** Chaque fichier retrouvé est lu de bout en bout : **Intact**, **Abîmé** (avec la raison : coupé, mélangé à un autre fichier, index manquant…) ou **Vide**.
- 👯 **Doublons masqués.** Un même fichier retrouvé plusieurs fois n'est affiché qu'une fois.
- 📀 **Images disque.** Ouvre une copie `.img` / `.dd` d'un disque fatigué (faite avec ddrescue, par exemple) pour ne pas toucher à l'original. Sans droits administrateur.
- 🗂️ **Tri et filtres** par type (photos, vidéos, musique, documents, archives), format, taille et nom.
- 💾 **Tous les supports.** Disques durs, SSD SATA et NVMe, cartes SD, microSD et CompactFlash, clés USB, disques externes, CD/DVD.
- 🕒 **En arrière-plan.** La fenêtre peut se ranger près de l'horloge pendant une longue recherche.
- 🛠️ **Mode expert.** Le journal complet de la recherche (et les lignes de PhotoRec avec le moteur classique).
- 🎨 **Thèmes de couleurs.** Clair ou sombre en un clic, et aussi Nord, Catppuccin, Tokyo Night, Solarized, Monokai et Gruvbox.
- 🔄 **Mises à jour sûres.** Quand une nouvelle version sort, un clic la télécharge, vérifie sa signature et l'installe.
- 🩺 **Rapport d'aide.** *Préparer un rapport…* enregistre un zip (système, disques, journal de la dernière recherche) à envoyer à la personne qui vous aide, avec les noms personnels masqués. Open Recovr ne l'envoie jamais lui-même.
- 🌍 **9 langues :** français, anglais, espagnol, allemand, italien, portugais du Brésil, néerlandais, polonais, ukrainien.
- 🔒 **Lecture seule.** Rien n'est jamais écrit sur le disque fouillé. Pas de compte, pas de télémétrie, pas besoin d'Internet : la seule connexion est la recherche de mises à jour (le numéro de la dernière version, demandé à GitHub), désactivable dans les Paramètres.

Un téléphone branché en USB ne peut pas être fouillé : il ne montre que les fichiers encore présents, et sa mémoire est chiffrée. L'application explique où chercher à la place : la corbeille de l'appli Galerie ou Photos, Google Photos, iCloud ou la carte microSD.

## Télécharger

Prenez la dernière version dans **[Releases](https://github.com/jlerouge/open-recovr/releases/latest)** :

| Système | Fichier | Comment le lancer |
|---|---|---|
| **Windows 10 / 11** (64 bits) | `open-recovr-…-windows-x64-setup.exe` ou `…-windows-x64.zip` | Lancez l'installeur (sans droits d'administrateur), ou décompressez et ouvrez `open-recovr.exe`. Les deux se mettent à jour en un clic. PhotoRec est facultatif : les Paramètres peuvent le télécharger pour le moteur classique. |
| Linux (64 bits) | `open-recovr-…-linux-x64.tar.gz` | Décompressez, puis lancez `./open-recovr`. Pour le moteur classique (facultatif) : `sudo apt install testdisk`. |
| macOS (Apple Silicon / Intel) | `open-recovr-…-macos-arm64.tar.gz` / `…-macos-x64.tar.gz` | Décompressez, puis clic droit → *Ouvrir* (l'application n'est pas signée). Pour le moteur classique (facultatif) : `brew install testdisk`. |

Lire le disque lui-même (analyse rapide et analyse complète) demande les droits administrateur. Open Recovr ne les demande que si vous passez en recherche approfondie ; la corbeille, elle, se fouille sans.

## Systèmes pris en charge

| | Windows | Linux | macOS |
|---|:---:|:---:|:---:|
| Compilé par la CI | ✅ | ✅ | ✅ |
| Testé sur une vraie machine | ✅ | ⚠️ pas encore | ⚠️ pas encore |
| Corbeille | ✅ | ✅ corbeille freedesktop | ✅ `.Trash` |
| Table des fichiers NTFS (analyse rapide) | ✅ | ✅ volumes NTFS | ✅ volumes NTFS |
| Analyse complète, moteur moderne | ✅ | ✅ | ✅ |
| Analyse complète, moteur classique (PhotoRec) | ✅ téléchargé depuis les Paramètres | ✅ `apt install testdisk` | ✅ `brew install testdisk` |
| Vignettes vidéo / PDF, lecture des sons | ✅ | ❌ | ❌ |
| Icône près de l'horloge | ✅ | ❌ | ❌ |

**Windows est la plateforme principale et testée.** Les versions Linux et macOS compilent, et leur code système est écrit, mais personne ne les a encore essayées sur une vraie machine. Retours et corrections bienvenus : voir [CONTRIBUTING.md](CONTRIBUTING.md).

## Compiler soi-même

Il faut [Rust](https://rustup.rs) 1.95 ou plus récent.

```sh
git clone https://github.com/jlerouge/open-recovr
cd open-recovr
cargo build --release
```

Le programme se trouve dans `target/release/`. Pour le moteur classique sous Windows, les Paramètres peuvent télécharger PhotoRec ; les développeurs peuvent aussi le récupérer une fois pour toutes :

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1
```

Les options en ligne de commande (`--lang`, `--theme`, `--disk`, `--image`, `--search`, `--expert`, `--screenshot`, `--scene`, `--bench`) sont décrites dans le [README anglais](README.md#build-from-source).

## Documentation

- 📖 [Guide d'utilisation](docs/user-guide.fr.md) : chaque étape, avec conseils et FAQ
- 🏗️ [Architecture](docs/architecture.md) : organisation du code et fonctionnement de chaque recherche (en anglais)
- 🔬 [Carving](docs/carving.md) : comment le moteur moderne trouve les fichiers et les recolle (en anglais)
- 📊 [Bancs d'essai](docs/benchmarks.md) : le moteur moderne et PhotoRec sur les mêmes disques (en anglais)
- 🤝 [Contribuer](CONTRIBUTING.md) : compiler, tester, traduire. Les contributions assistées par IA sont acceptées.
- 📝 [Changements](CHANGELOG.md)

## Licence et remerciements

Open Recovr est un logiciel libre sous [licence MIT](LICENSE).

Le moteur moderne d'Open Recovr, utilisé par défaut pour l'analyse complète, est écrit de zéro à partir des spécifications des formats de fichiers et ne partage aucun code avec PhotoRec. Ses méthodes sont décrites dans [docs/carving.md](docs/carving.md) (en anglais).

Le moteur classique lance **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)** © Christophe Grenier, sous licence GNU GPL v2 ou ultérieure, comme un **programme séparé, sans le modifier**. PhotoRec n'est pas fourni avec Open Recovr : sous Windows, quand vous le demandez dans les Paramètres, Open Recovr télécharge l'archive officielle de PhotoRec 7.2 depuis [cgsecurity.org](https://www.cgsecurity.org/wiki/TestDisk_Download) et vérifie chaque fichier avec un SHA-256 connu ; sous Linux et macOS, il vient du paquet `testdisk` du système. PhotoRec a posé les bases du file carving en open source, et ce projet lui doit beaucoup. Merci Christophe pour plus de vingt ans de travail sur TestDisk et PhotoRec.

Open Recovr est un projet indépendant, sans lien avec aucun logiciel de récupération commercial.

## Historique des étoiles

<a href="https://star-history.com/#jlerouge/open-recovr&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=jlerouge/open-recovr&type=Date&theme=dark">
    <img alt="Courbe des étoiles GitHub" src="https://api.star-history.com/svg?repos=jlerouge/open-recovr&type=Date">
  </picture>
</a>
