# Instrukcja obsługi

[English](user-guide.md) · [Français](user-guide.fr.md) · [Español](user-guide.es.md) · [Deutsch](user-guide.de.md) · [Italiano](user-guide.it.md) · [Português (Brasil)](user-guide.pt.md) · [Nederlands](user-guide.nl.md) · **Polski** · [Українська](user-guide.uk.md)

> [!NOTE]
> Ta instrukcja jest tłumaczeniem maszynowym, a zrzuty ekranu są po angielsku. Poprawki od rodzimego użytkownika języka są mile widziane: zobacz [CONTRIBUTING.md](../CONTRIBUTING.md).

Ta instrukcja prowadzi przez odzyskiwanie od początku do końca. Aplikacja mówi po polsku, angielsku, francusku, hiszpańsku, niemiecku, włosku, portugalsku (Brazylia), niderlandzku i ukraińsku: przycisk języka jest w prawym górnym rogu.

- [Zanim zaczniesz](#zanim-zaczniesz)
- [1. Szybkie czy głębokie wyszukiwanie](#1-szybkie-czy-głębokie-wyszukiwanie)
- [2. Wybór dysku i tego, czego szukasz](#2-wybór-dysku-i-tego-czego-szukasz)
- [3. Wyszukiwanie](#3-wyszukiwanie)
- [4. Przeglądanie wyników](#4-przeglądanie-wyników)
- [5. Zaznaczanie tego, co chcesz odzyskać](#5-zaznaczanie-tego-co-chcesz-odzyskać)
- [6. Odzyskiwanie](#6-odzyskiwanie)
- [Obrazy dysków](#obrazy-dysków)
- [Ustawienia, motywy i aktualizacje](#ustawienia-motywy-i-aktualizacje)
- [Tryb eksperta](#tryb-eksperta)
- [Prośba o pomoc](#prośba-o-pomoc)
- [Najczęstsze pytania](#najczęstsze-pytania)

## Zanim zaczniesz

**Używaj dysku jak najmniej.** Usunięty plik zostaje na dysku, dopóki nie zostanie nadpisany czymś nowym. Każde pobieranie, instalacja czy zapis na tym dysku zmniejsza Twoje szanse.

- Jeśli pliki były na dysku systemowym (zwykle `C:`), nie instaluj na nim niczego. Jeśli możesz, uruchom Open Recovr z pendrive'a.
- Jeśli były na karcie pamięci lub pendrivie, wyjmij go i nie rób nim nowych zdjęć.
- **Przygotuj inny dysk** na odzyskane pliki: pendrive, dysk zewnętrzny albo drugi dysk wewnętrzny.

## 1. Szybkie czy głębokie wyszukiwanie

![Okno „Jak głęboko szukać?”](screenshots/en-1-start.png)

Na starcie Open Recovr pyta, jak głęboko szukać:

- **Szybkie wyszukiwanie** sprawdza tylko Kosz. Trwa kilka sekund i nie wymaga specjalnych uprawnień. Jeśli opróżniłeś Kosz, niczego nie znajdzie.
- **Głębokie wyszukiwanie** czyta sam dysk. Wymaga uprawnień administratora: Windows prosi o zezwolenie aplikacji (w Linuksie i macOS pojawi się prośba o hasło).

Później możesz to zmienić przyciskiem **Przejdź do głębokiego wyszukiwania** u góry. Zaznacz *Nie proponuj tego więcej przy starcie*, jeśli zawsze wybierasz to samo.

## 2. Wybór dysku i tego, czego szukasz

![Ekran startowy](screenshots/en-2-home.png)

**Gdzie były Twoje pliki?** Kliknij dysk, kartę lub pendrive, na którym były pliki. Każdy pokazuje swój typ (dysk twardy, SSD SATA lub NVMe, karta SD, pendrive, CD…), wolne miejsce i nazwę.

**Szukam:** zawęża wyszukiwanie do *Zdjęć*, *Filmów*, *Muzyki*, *Dokumentów* lub *Archiwów*. Mniej typów to szybsze wyszukiwanie i krótsza lista. **Zawęź** idzie dalej: konkretne formaty (tylko JPG, tylko DOCX…) i minimalny rozmiar.

W głębokim wyszukiwaniu dyski NTFS (większość dysków Windows) oferują dwie analizy:

| Analiza | Co czyta | Czas | Zachowuje nazwy? |
|---|---|---|---|
| **Szybkie** | Tablicę plików dysku (MFT) | Kilka minut | ✅ nazwa i folder |
| **Pełne** | Tablicę plików, a potem cały dysk za pomocą PhotoRec | Od minut do ponad godziny | Częściowo (patrz niżej) |

Zacznij od **Szybkie**. Jeśli Twojego pliku tam nie ma, kliknij **Nie znaleziono? Uruchom pełne skanowanie**.

W innych systemach plików (karty pamięci FAT32 i exFAT, ext4, APFS…) głębokie wyszukiwanie od razu przechodzi do pełnej analizy.

## 3. Wyszukiwanie

![Trwające głębokie wyszukiwanie](screenshots/en-3-scanning.png)

Kliknij **Szukaj utraconych plików**. Pliki pojawiają się na liście, **gdy tylko zostaną znalezione**: nie musisz czekać do końca, żeby zacząć przeglądać.

Panel po prawej pokazuje bieżący etap, postęp, szacowany pozostały czas i liczbę znalezionych dotąd plików.

- **Zatrzymaj wyszukiwanie** zachowuje to, co już znaleziono.
- **Kontynuuj w tle** chowa okno obok zegara (Windows). Kliknij ikonę, aby je przywrócić. Powiadomienie poinformuje Cię o końcu wyszukiwania.

Nieczytelne obszary, na przykład na porysowanej płycie CD lub zużytej karcie, są pomijane. Wyszukiwanie trwa dalej, zamiast się zawiesić.

## 4. Przeglądanie wyników

![Wyniki z podglądem](screenshots/en-4-results.png)

- **Karty** u góry (*Wszystko*, *Zdjęcia*, *Dokumenty*…) pokazują, ile plików każdego rodzaju znaleziono.
- **Format** i **Rozmiar** filtrują dalej. Pole wyszukiwania filtruje po nazwie.
- Kliknij tytuł kolumny, aby **posortować** według typu, nazwy, daty lub rozmiaru.
- Kliknij plik, aby zobaczyć jego **podgląd** po prawej, razem ze szczegółami. Można podejrzeć zdjęcia, filmy, dźwięki, pliki PDF, teksty i dokumenty Word. **Otwórz** otwiera go w zwykłym programie, z tymczasowej kopii.

Każdy znaleziony plik jest czytany od początku do końca, a kolumna **Stan** podaje wynik:

| Stan | Znaczenie |
|---|---|
| **Nienaruszony** | Plik sprawdzono od początku do końca i jest kompletny. Wróci taki, jaki był. |
| **Uszkodzony** | Brakuje części lub jest błędna. Szczegóły mówią dlaczego: ucięty, pomieszany z innym plikiem, brak indeksu… Może jeszcze częściowo się otworzyć. |
| **Pusty** | Miejsce, które zajmował, zawiera same zera. Nie ma już czego odzyskać. |

Ten sam plik często jest znajdowany kilka razy (na przykład w Koszu i na dysku). Kopie są ukryte za linkiem *Ukryte duplikaty · Pokaż*.

Pliki znalezione przez PhotoRec nie mają własnej nazwy. Open Recovr nadaje im ją na podstawie zawartości, gdy to możliwe: `photo_2024-07-14_18-22-05.jpg` z daty zrobienia zdjęcia, tytuł dokumentu Word lub PDF, wykonawca i tytuł utworu MP3.

## 5. Zaznaczanie tego, co chcesz odzyskać

![Zdjęcia zaznaczone do odzyskania](screenshots/en-5-selected.png)

Zaznaczaj pliki pojedynczo albo użyj pola w nagłówku kolumny, aby zaznaczyć wszystko w bieżącym widoku. **Zaznacz wszystko** i **Odznacz wszystko** na dole działają na całej liście. Dolny pasek pokazuje, ile plików jest zaznaczonych i jaki jest ich łączny rozmiar.

## 6. Odzyskiwanie

![Odzyskiwanie zakończone](screenshots/en-6-recovered.png)

Kliknij **Odzyskaj zaznaczone** i wybierz folder **na innym dysku**. Jeśli wybierzesz przeszukiwany dysk, Open Recovr ostrzeże, że możesz nadpisać inne pliki do odzyskania.

Pliki są kopiowane do nowego folderu *Odzyskane pliki* z datą, z podfolderem dla każdego rodzaju (Zdjęcia, Dokumenty…). Folder usunięty do Kosza wraca w całości. Na koniec **Otwórz folder** je pokazuje.

Na przeszukiwanym dysku nigdy nic nie jest zapisywane. Pliki są z niego tylko czytane.

## Obrazy dysków

Psujący się dysk (stuka, jest bardzo wolny, pełen błędów odczytu) może całkiem odmówić posłuszeństwa w trakcie wyszukiwania. Bezpieczniej jest skopiować go raz, sektor po sektorze, narzędziem takim jak [ddrescue](https://www.gnu.org/software/ddrescue/), a potem przeszukać kopię.

Karta **Obraz dysku**, obok dysków, otwiera taką kopię (`.img`, `.dd`, `.raw`…). Wyszukiwanie działa wtedy jak na dysku, bez uprawnień administratora, a oryginał pozostaje nietknięty.

## Ustawienia, motywy i aktualizacje

![Strona ustawień](screenshots/en-8-settings.png)

Menu w prawym górnym rogu (☰) zawiera wszystko, co nie jest potrzebne bezpośrednio do odzyskiwania plików:

- **Ustawienia**: język, motyw kolorów, tryb eksperta, czy proponować głębokie wyszukiwanie przy starcie, aktualizacje i informacje o aplikacji.
- **Motyw kolorów**: jasny, ciemny, Nord, Catppuccin, Tokyo Night, Solarized, Monokai lub Gruvbox. Przycisk słońce/księżyc obok języka przełącza jednym kliknięciem między jasnym a ciemnym.
- **Sprawdź aktualizacje**, **Przygotuj raport…** i strona projektu na GitHubie.

Gdy wyjdzie nowa wersja, dolny pasek pokazuje **→ v…: aktualizuj**. Jedno kliknięcie pobiera ją, sprawdza, czy jest podpisana kluczem projektu, instaluje i ponownie uruchamia Open Recovr. Jeśli trwa wyszukiwanie lub odzyskiwanie, aktualizacja czeka na jego zakończenie.

## Tryb eksperta

![Tryb eksperta z dziennikiem wyszukiwania](screenshots/en-7-expert.png)

**Tryb eksperta**, w menu w prawym górnym rogu, pokazuje pod listą dziennik wyszukiwania: dysk i jego system plików, każdy etap i wiersze PhotoRec. **Kopiuj wszystko** kopiuje dziennik, co przydaje się przy zgłaszaniu błędu. Przeciągnij górną krawędź dziennika, aby go powiększyć.

Każde wyszukiwanie jest też zapisywane w `journal.txt`, obok ustawień (`%APPDATA%\Open Recovr` w Windows). Link **Dziennik** na dole otwiera ten plik.

## Prośba o pomoc

W menu w prawym górnym rogu (☰) **Przygotuj raport…** zapisuje plik zip, który możesz wysłać osobie, która Ci pomaga, e-mailem lub w inny sposób. Open Recovr tylko zapisuje plik; nigdy go nie wysyła. W środku są pliki tekstowe: czym jest raport, system i dyski (model, rozmiar, system plików), ustawienia i dziennik ostatniego wyszukiwania. Zanim cokolwiek zostanie zapisane, Twoja nazwa konta, nazwa komputera, Twój folder osobisty i adresy e-mail są maskowane, podobnie jak tytuły dokumentów, które PhotoRec umieszcza w nazwach znalezionych plików. Otwórz zip i przeczytaj go, zanim go przekażesz.

## Najczęstsze pytania

**Mojego pliku nie ma na liście. Co teraz?**
Sprawdź, czy wybrano właściwy dysk i czy filtr typu go nie ukrywa (*Wszystko*). Potem spróbuj głębokiego wyszukiwania, a na NTFS pełnej analizy. Jeśli dysk był intensywnie używany od usunięcia, plik mógł zostać nadpisany. Wtedy żaden program go nie przywróci.

**Odzyskane pliki nazywają się `f1234567.jpg`.**
Te pliki znalazł PhotoRec, który rozpoznaje plik po zawartości, a nie po tablicy plików: ich oryginalna nazwa jest nieznana. Gdy sam plik nic nie mówi o swojej dacie ani tytule, numer zostaje. Posortuj według daty lub rozmiaru i użyj podglądu, aby znaleźć swój.

**Plik jest oznaczony jako Uszkodzony. Czy warto go odzyskać?**
Często tak. Uszkodzone zdjęcie może pokazać tylko górną połowę, uszkodzony film może grać do uszkodzonego fragmentu. Odzyskaj go i spróbuj: nic to nie kosztuje, a dysk nigdy nie jest zmieniany.

**Czy może odzyskać pliki z mojego telefonu?**
Nie przez USB. Telefon udostępnia tylko pliki, które jeszcze ma, a jego pamięć jest zaszyfrowana. Zajrzyj do kosza aplikacji Galeria lub Zdjęcia (usunięte zdjęcia zwykle zostają tam 30 dni), do Zdjęć Google lub iCloud albo włóż kartę microSD telefonu do czytnika kart i przeszukaj ją.

**Czy działa na dysku SSD?**
Częściowo. Większość dysków SSD kasuje usunięte bloki w tle (TRIM): pliki usunięte jakiś czas temu często przepadły na dobre. Kosz i tablica plików nadal pomagają przy niedawnych usunięciach. Działaj szybko.

**Czy można odzyskać pliki ze sformatowanego dysku?**
Często tak, za pomocą pełnej analizy. Szybkie formatowanie zostawia zawartość na miejscu. Pełne formatowanie (lub „bezpieczne wymazywanie”) ją nadpisuje.

**Dlaczego głębokie wyszukiwanie wymaga uprawnień administratora?**
Czyta dysk sektor po sektorze, poniżej systemu plików, a system operacyjny zastrzega to dla administratorów. Open Recovr tylko czyta; nigdy nie zapisuje na tym dysku.

**Czy to bezpieczne? Czy wysyła gdzieś moje dane?**
Open Recovr działa offline. Bez konta, bez telemetrii. Jedyne połączenie to sprawdzanie aktualizacji: przy starcie pyta GitHub o numer najnowszej wersji i nie wysyła nic więcej. Odznacz „Sprawdzaj aktualizacje przy starcie” w Ustawieniach, aby je wyłączyć; menu nadal może sprawdzić na żądanie. Aktualizacja jest instalowana tylko wtedy, gdy jest podpisana kluczem projektu, który nigdy nie trafia na GitHub: fałszywa wersja wrzucona tam przez kogoś innego zostaje odrzucona. Kod jest otwarty: sprawdź sam.

**Linux / macOS: głębokie wyszukiwanie mówi, że PhotoRec jest niedostępny.**
Zainstaluj go: `sudo apt install testdisk` (Debian, Ubuntu), `sudo dnf install testdisk` (Fedora) lub `brew install testdisk` (macOS). Open Recovr sam znajdzie wtedy `photorec`. Bez niego prostszy wbudowany silnik szuka najpopularniejszych formatów.
