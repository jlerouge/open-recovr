<div align="center">

<img src="assets/icon.svg" alt="" width="96" height="96">

# Open Recovr

**Поверніть видалені файли, просто.**

Один екран, без майстра: оберіть диск, шукайте й позначте те, що хочете повернути.<br>
Вільна й безкоштовна програма на основі [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec).

[English](README.md) · [Français](README.fr.md) · [Español](README.es.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [Português (Brasil)](README.pt.md) · [Nederlands](README.nl.md) · [Polski](README.pl.md) · **Українська**

[![Tests](https://img.shields.io/github/actions/workflow/status/jlerouge/open-recovr/ci.yml?branch=main&label=tests&logo=github)](https://github.com/jlerouge/open-recovr/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/jlerouge/open-recovr?include_prereleases&sort=semver)](https://github.com/jlerouge/open-recovr/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jlerouge/open-recovr/total)](https://github.com/jlerouge/open-recovr/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)](#підтримувані-системи)
[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

<img src="docs/screenshots/en-4-results.png" alt="Open Recovr показує видалені файли, знайдені на диску C:, з попереднім переглядом фото" width="820">

</div>

> [!NOTE]
> Цю сторінку перекладено машинно, а знімки екрана англійською. Виправлення від носія мови вітаються: див. [CONTRIBUTING.md](CONTRIBUTING.md).

## Чому Open Recovr?

Втратити папку з відпустки чи дипломну роботу — вже достатній стрес. Програми для відновлення часто дорогі або переповнені жаргоном. Безкоштовні, як-от PhotoRec, чудові, але працюють у текстовому терміналі.

Open Recovr додає до PhotoRec, перевіреного вільного рушія, дружній інтерфейс на одному екрані. А ще — швидкий аналіз, що повертає файли **з їхньою початковою назвою і папкою**.

## Як це працює

| 1. Швидко чи глибоко | 2. Оберіть диск | 3. Пошук |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-1-start.png)](docs/screenshots/en-1-start.png) | [![](docs/screenshots/en-2-home.png)](docs/screenshots/en-2-home.png) | [![](docs/screenshots/en-3-scanning.png)](docs/screenshots/en-3-scanning.png) |
| Кілька секунд у Кошику або весь диск. | Жорсткий диск, SSD, картка пам'яті, USB-флешка, CD… | Файли з'являються одразу, як їх знайдено, з часом, що лишився. |

| 4. Перегляд | 5. Позначте | 6. Відновіть |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-4-results.png)](docs/screenshots/en-4-results.png) | [![](docs/screenshots/en-5-selected.png)](docs/screenshots/en-5-selected.png) | [![](docs/screenshots/en-6-recovered.png)](docs/screenshots/en-6-recovered.png) |
| Фільтри за типом, форматом, розміром чи назвою. | Кожен файл показує, чи він цілий. | Копіюються на обраний вами диск. |

| Налаштування | Темна тема | Режим експерта |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-8-settings.png)](docs/screenshots/en-8-settings.png) | [![](docs/screenshots/en-9-dark.png)](docs/screenshots/en-9-dark.png) | [![](docs/screenshots/en-7-expert.png)](docs/screenshots/en-7-expert.png) |
| Мова, колірна тема, оновлення, звіт. | Світла, темна та ще 8 тем. | Журнал пошуку, для допитливих. |

Повний покроковий опис — у **[посібнику користувача](docs/user-guide.uk.md)**.

> [!WARNING]
> **Ніколи не зберігайте відновлені файли на диск, з якого вони походять.** Ви можете перезаписати файли, які ще чекають на відновлення. USB-флешка чи інший диск — ідеальний варіант. Open Recovr попередить, якщо ви спробуєте.

## Можливості

- 🗑️ **Миттєвий пошук** у Кошику, за кілька секунд і без особливих прав.
- 🔎 **Швидкий аналіз (NTFS).** Читає таблицю файлів диска (MFT), де видалений файл зберігає свою назву. Файли повертаються **з назвою і папкою**, за кілька хвилин, навіть після очищення Кошика.
- 🧲 **Повний аналіз.** PhotoRec читає весь диск і розпізнає **сотні типів файлів**, навіть після форматування і на картках FAT/exFAT. Без PhotoRec найпоширенішими форматами займається вбудований рушій.
- 💿 **Пошкоджені носії.** Нечитабельні ділянки (подряпані CD/DVD, зношені картки) пропускаються, а не блокують пошук.
- 🖼️ **Попередній перегляд перед відновленням.** Фото, відео, звуки, PDF, тексти й документи Word.
- 🏷️ **Змістовні назви й дати.** Файл, що мав лише номер, отримує назву зі свого вмісту: `photo_2024-07-14_18-22-05.jpg` за датою EXIF, назва документа Word чи PDF, виконавець і пісня MP3.
- ✅ **Перевірено перед відновленням.** Кожен знайдений файл читається від початку до кінця: **Цілий**, **Пошкоджений** (з причиною: обрізаний, змішаний з іншим файлом, бракує індексу…) або **Порожній**.
- 👯 **Дублікати приховано.** Той самий файл, знайдений кілька разів, показується один раз.
- 📀 **Образи дисків.** Відкриває копію `.img` / `.dd` диска, що виходить з ладу (зроблену, наприклад, за допомогою ddrescue), щоб не чіпати оригінал. Без прав адміністратора.
- 🗂️ **Сортування та фільтри** за типом (фото, відео, музика, документи, архіви), форматом, розміром і назвою.
- 💾 **Будь-які диски.** Жорсткі диски, SSD SATA і NVMe, картки SD, microSD і CompactFlash, USB-флешки, зовнішні диски, CD/DVD.
- 🕒 **У фоні.** Під час довгого пошуку вікно може сховатися біля годинника.
- 🛠️ **Режим експерта.** Повний журнал пошуку і вивід PhotoRec.
- 🎨 **Колірні теми.** Світла чи темна одним клацанням, а також Nord, Catppuccin, Tokyo Night, Solarized, Monokai і Gruvbox.
- 🔄 **Безпечні оновлення.** Коли виходить нова версія, одне клацання завантажує її, перевіряє підпис і встановлює.
- 🩺 **Звіт для допомоги.** *Підготувати звіт…* зберігає zip (система, диски, журнал останнього пошуку), щоб надіслати людині, яка вам допомагає, з прихованими особистими іменами. Open Recovr ніколи не надсилає його сам.
- 🌍 **9 мов:** українська, англійська, французька, іспанська, німецька, італійська, бразильська португальська, нідерландська та польська.
- 🔒 **Лише читання.** На диск, де ведеться пошук, ніколи нічого не записується. Без облікового запису, без телеметрії, без потреби в інтернеті: єдине з'єднання — перевірка оновлень (номер останньої версії, запитаний у GitHub), яку можна вимкнути в Налаштуваннях.

Телефон, під'єднаний через USB, не можна просканувати: він показує лише файли, які в нього ще є, а його пам'ять зашифрована. Open Recovr пояснює, де шукати натомість: у кошику застосунку Галерея чи Фото, у Google Фото, в iCloud або на картці microSD.

## Завантаження

Завантажте останню версію з **[Releases](https://github.com/jlerouge/open-recovr/releases/latest)**:

| Система | Файл | Як запустити |
|---|---|---|
| **Windows 10 / 11** (64-біт) | `open-recovr-…-windows-x64-setup.exe` або `…-windows-x64.zip` | Запустіть інсталятор (без прав адміністратора) або розпакуйте й відкрийте `open-recovr.exe`. PhotoRec додано. Обидва варіанти оновлюються одним клацанням. |
| Linux (64-біт) | `open-recovr-…-linux-x64.tar.gz` | Розпакуйте й запустіть `./open-recovr`. Для повного аналізу: `sudo apt install testdisk`. |
| macOS (Apple Silicon / Intel) | `open-recovr-…-macos-arm64.tar.gz` / `…-macos-x64.tar.gz` | Розпакуйте, потім клацніть правою кнопкою → *Open* (застосунок не підписаний). Далі `brew install testdisk`. |

Читання самого диска (швидкий і повний аналіз) потребує прав адміністратора. Open Recovr просить їх лише тоді, коли ви переходите до глибокого пошуку; Кошик перевіряється без них.

## Підтримувані системи

| | Windows | Linux | macOS |
|---|:---:|:---:|:---:|
| Збирається в CI | ✅ | ✅ | ✅ |
| Перевірено на справжньому комп'ютері | ✅ | ⚠️ ще ні | ⚠️ ще ні |
| Кошик | ✅ | ✅ кошик freedesktop | ✅ `.Trash` |
| Таблиця файлів NTFS (швидкий аналіз) | ✅ | ✅ томи NTFS | ✅ томи NTFS |
| Повний аналіз (PhotoRec) | ✅ додано | ✅ `apt install testdisk` | ✅ `brew install testdisk` |
| Мініатюри відео / PDF, звук | ✅ | ❌ | ❌ |
| Значок біля годинника | ✅ | ❌ | ❌ |

**Windows — основна, перевірена платформа.** Версії для Linux і macOS компілюються, але ще ніхто не перевірив їх на справжньому комп'ютері. Повідомлення й виправлення вітаються: див. [CONTRIBUTING.md](CONTRIBUTING.md).

## Збирання

Потрібен [Rust](https://rustup.rs) 1.95 або новіший.

```sh
git clone https://github.com/jlerouge/open-recovr
cd open-recovr
cargo build --release
```

У Windows один раз завантажте PhotoRec командою `powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1`. Параметри командного рядка описано в [англійському README](README.md#build-from-source).

## Ліцензія та подяки

Open Recovr — вільне програмне забезпечення за [ліцензією MIT](LICENSE).

Повний аналіз виконує **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)** © Christophe Grenier, за ліцензією GNU GPL v2 або новішою. Open Recovr запускає його як **окрему, незмінену програму**. Версія для Windows містить PhotoRec 7.2 з його ліцензією (`vendor/testdisk-7.2/COPYING.txt`). Вихідний код PhotoRec доступний на [cgsecurity.org](https://www.cgsecurity.org/wiki/TestDisk_Download).

Open Recovr — незалежний проєкт, не пов'язаний із жодним комерційним програмним забезпеченням для відновлення.
