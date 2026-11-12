# Gebruikershandleiding

[English](user-guide.md) · [Français](user-guide.fr.md) · [Español](user-guide.es.md) · [Deutsch](user-guide.de.md) · [Italiano](user-guide.it.md) · [Português (Brasil)](user-guide.pt.md) · **Nederlands** · [Polski](user-guide.pl.md) · [Українська](user-guide.uk.md)

> [!NOTE]
> Deze handleiding is machinaal vertaald en de schermafbeeldingen zijn in het Engels. Verbeteringen van een moedertaalspreker zijn welkom: zie [CONTRIBUTING.md](../CONTRIBUTING.md).

Deze handleiding loopt een herstel van begin tot eind door. De app spreekt Nederlands, Engels, Frans, Spaans, Duits, Italiaans, Braziliaans Portugees, Pools en Oekraïens: de taalknop staat rechtsboven.

- [Voordat u begint](#voordat-u-begint)
- [1. Snel of diep zoeken](#1-snel-of-diep-zoeken)
- [2. De schijf kiezen en wat u zoekt](#2-de-schijf-kiezen-en-wat-u-zoekt)
- [3. Het zoeken](#3-het-zoeken)
- [4. De resultaten bekijken](#4-de-resultaten-bekijken)
- [5. Aanvinken wat u terug wilt](#5-aanvinken-wat-u-terug-wilt)
- [6. Herstellen](#6-herstellen)
- [Schijfkopieën](#schijfkopieën)
- [Instellingen, thema's en updates](#instellingen-themas-en-updates)
- [Expertmodus](#expertmodus)
- [Iemand om hulp vragen](#iemand-om-hulp-vragen)
- [Veelgestelde vragen](#veelgestelde-vragen)

## Voordat u begint

**Gebruik de schijf zo weinig mogelijk.** Een verwijderd bestand blijft op de schijf staan tot er iets nieuws overheen wordt geschreven. Elke download, installatie of opslag op die schijf verkleint uw kansen.

- Stonden de bestanden op de systeemschijf (meestal `C:`), installeer daar dan niets. Start Open Recovr zo mogelijk vanaf een USB-stick.
- Stonden ze op een geheugenkaart of USB-stick, haal die er dan uit en maak er geen nieuwe foto's meer mee.
- **Houd een andere schijf klaar** voor de herstelde bestanden: een USB-stick, een externe schijf of een tweede interne schijf.

## 1. Snel of diep zoeken

![Het venster „Hoe ver zoeken?”](screenshots/en-1-start.png)

Bij het starten vraagt Open Recovr hoe ver er gezocht moet worden:

- **Snel zoeken** kijkt alleen in de Prullenbak. Het duurt een paar seconden en vraagt geen bijzondere rechten. Als u de Prullenbak hebt geleegd, vindt het niets.
- **Diep zoeken** leest de schijf zelf. Daarvoor zijn beheerdersrechten nodig: Windows vraagt u de app toe te staan (op Linux en macOS wordt om uw wachtwoord gevraagd).

U kunt later wisselen met de knop **Overschakelen naar diep zoeken** bovenaan. Vink *Niet meer aanbieden bij het starten* aan als u altijd hetzelfde kiest.

## 2. De schijf kiezen en wat u zoekt

![Het beginscherm](screenshots/en-2-home.png)

**Waar stonden uw bestanden?** Klik op de schijf, kaart of stick waar de bestanden stonden. Elk toont het type (harde schijf, SATA- of NVMe-SSD, SD-kaart, USB-stick, cd…), de vrije ruimte en de naam.

**Ik zoek:** beperkt het zoeken tot *Foto's*, *Video's*, *Muziek*, *Documenten* of *Archieven*. Minder soorten maken het zoeken sneller en de lijst korter. **Verfijnen** gaat verder: bepaalde formaten (alleen JPG, alleen DOCX…) en een minimale grootte.

Bij diep zoeken bieden NTFS-schijven (de meeste Windows-schijven) twee analyses:

| Analyse | Wat ze leest | Duur | Namen behouden? |
|---|---|---|---|
| **Snel** | De bestandstabel van de schijf (MFT) | Een paar minuten | ✅ naam en map |
| **Volledig** | De bestandstabel, daarna de hele schijf met PhotoRec | Van minuten tot meer dan een uur | Deels (zie hieronder) |

Begin met **Snel**. Staat uw bestand er niet bij, klik dan op **Niet gevonden? Start de volledige scan**.

Op andere bestandssystemen (geheugenkaarten met FAT32 en exFAT, ext4, APFS…) gaat diep zoeken meteen naar de volledige analyse.

## 3. Het zoeken

![Diep zoeken bezig](screenshots/en-3-scanning.png)

Klik op **Verloren bestanden zoeken**. Bestanden verschijnen in de lijst **zodra ze gevonden zijn**: u hoeft niet op het einde te wachten om te gaan kijken.

Het paneel rechts toont de huidige fase, de voortgang, een schatting van de resterende tijd en hoeveel bestanden er tot nu toe gevonden zijn.

- **Zoeken stoppen** houdt wat al gevonden is.
- **Doorgaan op de achtergrond** zet het venster naast de klok (Windows). Klik op het pictogram om het terug te halen. Een melding laat weten wanneer het zoeken klaar is.

Onleesbare gebieden, zoals op een bekraste cd of een versleten kaart, worden overgeslagen. Het zoeken gaat door in plaats van vast te lopen.

## 4. De resultaten bekijken

![De resultaten, met een voorbeeld](screenshots/en-4-results.png)

- De **tabbladen** bovenaan (*Alles*, *Foto's*, *Documenten*…) tonen hoeveel bestanden van elke soort er gevonden zijn.
- **Formaat** en **Grootte** filteren verder. Het zoekvak filtert op naam.
- Klik op een kolomtitel om te **sorteren** op type, naam, datum of grootte.
- Klik op een bestand om rechts een **voorbeeld** en de details te zien. Foto's, video's, geluiden, pdf's, teksten en Word-documenten kunnen worden bekeken. **Openen** opent het in het gebruikelijke programma, vanuit een tijdelijke kopie.

Elk gevonden bestand wordt van begin tot eind gelezen, en de kolom **Staat** geeft het resultaat:

| Staat | Betekenis |
|---|---|
| **Intact** | Het bestand is van begin tot eind gecontroleerd en is compleet. Het komt terug zoals het was. |
| **Beschadigd** | Een deel ontbreekt of klopt niet. De details zeggen waarom: afgekapt, vermengd met een ander bestand, index ontbreekt… Het opent misschien nog gedeeltelijk. |
| **Leeg** | De ruimte die het innam bevat alleen nullen. Er valt niets meer te herstellen. |

Hetzelfde bestand wordt vaak meerdere keren gevonden (in de Prullenbak en op de schijf, bijvoorbeeld). De kopieën zitten verborgen achter een link *duplicaten verborgen · Tonen*.

Door PhotoRec gevonden bestanden hebben geen eigen naam. Open Recovr geeft ze, als het kan, een naam op basis van de inhoud: `photo_2024-07-14_18-22-05.jpg` uit de opnamedatum van een foto, de titel van een Word- of pdf-document, artiest en nummer van een MP3.

## 5. Aanvinken wat u terug wilt

![Foto's aangevinkt om te herstellen](screenshots/en-5-selected.png)

Vink de bestanden een voor een aan, of gebruik het vakje in de kolomkop om alles in de huidige weergave aan te vinken. **Alles aanvinken** en **Alles uitvinken** onderaan werken op de hele lijst. De balk onderaan toont hoeveel bestanden aangevinkt zijn en hun totale grootte.

## 6. Herstellen

![Herstel voltooid](screenshots/en-6-recovered.png)

Klik op **Selectie herstellen** en kies een map **op een andere schijf**. Kiest u de doorzochte schijf, dan waarschuwt Open Recovr dat u andere herstelbare bestanden kunt overschrijven.

De bestanden worden gekopieerd naar een nieuwe map *Herstelde bestanden* met de datum erachter, met een submap per soort (Foto's, Documenten…). Een map die naar de Prullenbak is verplaatst, komt in zijn geheel terug. Na afloop toont **Map openen** ze.

Er wordt nooit iets naar de doorzochte schijf geschreven. De bestanden worden er alleen van gelezen.

## Schijfkopieën

Een schijf die het begeeft (tikt, is erg traag, zit vol leesfouten) kan tijdens het zoeken definitief uitvallen. Veiliger is het om hem één keer sector voor sector te kopiëren met een hulpmiddel zoals [ddrescue](https://www.gnu.org/software/ddrescue/), en daarna de kopie te doorzoeken.

De kaart **Schijfkopie** naast de schijven opent zo'n kopie (`.img`, `.dd`, `.raw`…). Het zoeken werkt dan als op een schijf, zonder beheerdersrechten, en het origineel blijft onaangeroerd.

## Instellingen, thema's en updates

![De pagina Instellingen](screenshots/en-8-settings.png)

Het menu rechtsboven (☰) bevat alles wat niet direct nodig is om bestanden terug te krijgen:

- **Instellingen**: de taal, het kleurthema, de expertmodus, of diep zoeken bij het starten wordt aangeboden, updates en informatie over de app.
- **Kleurthema**: licht, donker, Nord, Catppuccin, Tokyo Night, Solarized, Monokai of Gruvbox. De zon/maan-knop naast de taal wisselt met één klik tussen licht en donker.
- **Naar updates zoeken**, **Rapport maken…** en de projectpagina op GitHub.

Als er een nieuwe versie is, toont de balk onderaan **→ v…: bijwerken**. Eén klik downloadt hem, controleert of hij ondertekend is met de sleutel van het project, installeert hem en start Open Recovr opnieuw. Loopt er een zoekactie of herstel, dan wacht de update tot die klaar is.

## Expertmodus

![De expertmodus met het logboek van de zoekactie](screenshots/en-7-expert.png)

De **Expertmodus**, in het menu rechtsboven, toont onder de lijst het logboek van de zoekactie: de schijf en het bestandssysteem, elke fase en de regels van PhotoRec. **Alles kopiëren** kopieert het logboek, handig voor een foutmelding. Sleep de bovenrand van het logboek om het groter te maken.

Elke zoekactie wordt ook weggeschreven in `journal.txt`, naast de instellingen (`%APPDATA%\Open Recovr` op Windows). De link **Logboek** onderaan opent het.

## Iemand om hulp vragen

In het menu rechtsboven (☰) slaat **Rapport maken…** een zip op die u kunt sturen naar wie u helpt, per e-mail of op een andere manier. Open Recovr schrijft alleen het bestand; het verstuurt het nooit. Erin staan tekstbestanden: wat het rapport is, het systeem en de schijven (model, grootte, bestandssysteem), de instellingen en het logboek van de laatste zoekactie. Voordat er iets wordt geschreven, worden uw accountnaam, de naam van de computer, uw persoonlijke map en e-mailadressen onherkenbaar gemaakt, net als de documenttitels die PhotoRec in de namen van gevonden bestanden zet. Open de zip en lees hem door voordat u hem doorgeeft.

## Veelgestelde vragen

**Mijn bestand staat niet in de lijst. Wat nu?**
Controleer of de juiste schijf gekozen is en of het typefilter het niet verbergt (*Alles*). Probeer daarna diep zoeken en op NTFS de volledige analyse. Is de schijf na het verwijderen veel gebruikt, dan is het bestand misschien overschreven. Dan kan geen enkel programma het terughalen.

**De herstelde bestanden heten `f1234567.jpg`.**
Die bestanden zijn gevonden door PhotoRec, dat een bestand herkent aan de inhoud en niet via de bestandstabel: hun oorspronkelijke naam is onbekend. Als het bestand zelf niets zegt over de datum of de titel, blijft het nummer staan. Sorteer op datum of grootte en gebruik het voorbeeld om het uwe te vinden.

**Een bestand staat als Beschadigd. Is het de moeite waard om het te herstellen?**
Vaak wel. Een beschadigde foto toont misschien alleen de bovenste helft, een beschadigde video speelt misschien tot het kapotte stuk. Herstel het en probeer het: het kost niets, en de schijf wordt nooit gewijzigd.

**Kan het bestanden van mijn telefoon herstellen?**
Niet via USB. Een telefoon deelt alleen de bestanden die hij nog heeft, en zijn geheugen is versleuteld. Kijk in de prullenbak van de app Galerij of Foto's (verwijderde foto's blijven daar meestal 30 dagen), in Google Foto's of iCloud, of stop de microSD-kaart van de telefoon in een kaartlezer en doorzoek die.

**Werkt het op een SSD?**
Deels. De meeste SSD's wissen verwijderde blokken op de achtergrond (TRIM): bestanden die al een tijd geleden verwijderd zijn, zijn vaak voorgoed weg. De Prullenbak en de bestandstabel helpen nog bij recent verwijderde bestanden. Wees er snel bij.

**Kan ik bestanden van een geformatteerde schijf herstellen?**
Vaak wel, met de volledige analyse. Snel formatteren laat de inhoud staan. Volledig formatteren (of „veilig wissen”) overschrijft hem.

**Waarom heeft diep zoeken beheerdersrechten nodig?**
Het leest de schijf sector voor sector, onder het bestandssysteem, en het besturingssysteem houdt dat voor beheerders. Open Recovr leest alleen; het schrijft nooit naar die schijf.

**Is het veilig? Stuurt het mijn gegevens ergens heen?**
Open Recovr werkt offline. Geen account, geen telemetrie. De enige verbinding is het zoeken naar updates: bij het starten vraagt het GitHub naar het nummer van de nieuwste versie, en stuurt verder niets. Vink in Instellingen „Bij het starten naar updates zoeken” uit om dat uit te zetten; via het menu kunt u nog steeds op verzoek controleren. Een update wordt alleen geïnstalleerd als hij ondertekend is met de sleutel van het project, die nooit naar GitHub gaat: een nepversie die iemand anders daar neerzet, wordt geweigerd. De code is open: kijk zelf.

**Linux / macOS: diep zoeken meldt dat PhotoRec niet beschikbaar is.**
Installeer het: `sudo apt install testdisk` (Debian, Ubuntu), `sudo dnf install testdisk` (Fedora) of `brew install testdisk` (macOS). Open Recovr vindt `photorec` dan zelf. Zonder PhotoRec zoekt een eenvoudigere ingebouwde engine naar de meest voorkomende formaten.
