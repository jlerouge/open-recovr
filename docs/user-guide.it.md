# Guida all'uso

[English](user-guide.md) · [Français](user-guide.fr.md) · [Español](user-guide.es.md) · [Deutsch](user-guide.de.md) · **Italiano** · [Português (Brasil)](user-guide.pt.md) · [Nederlands](user-guide.nl.md) · [Polski](user-guide.pl.md) · [Українська](user-guide.uk.md)

> [!NOTE]
> Questa guida è una traduzione automatica e le schermate sono in inglese. Le correzioni di un madrelingua sono benvenute: vedi [CONTRIBUTING.md](../CONTRIBUTING.md).

Questa guida accompagna un recupero dall'inizio alla fine. L'applicazione parla italiano, inglese, francese, spagnolo, tedesco, portoghese brasiliano, olandese, polacco e ucraino: il pulsante della lingua è in alto a destra.

- [Prima di iniziare](#prima-di-iniziare)
- [1. Ricerca rapida o approfondita](#1-ricerca-rapida-o-approfondita)
- [2. Scegliere l'unità e cosa cerchi](#2-scegliere-lunità-e-cosa-cerchi)
- [3. La ricerca](#3-la-ricerca)
- [4. Sfogliare i risultati](#4-sfogliare-i-risultati)
- [5. Selezionare ciò che vuoi recuperare](#5-selezionare-ciò-che-vuoi-recuperare)
- [6. Recuperare](#6-recuperare)
- [Immagini disco](#immagini-disco)
- [Impostazioni, temi e aggiornamenti](#impostazioni-temi-e-aggiornamenti)
- [Modalità esperto](#modalità-esperto)
- [Chiedere aiuto a qualcuno](#chiedere-aiuto-a-qualcuno)
- [Domande frequenti](#domande-frequenti)

## Prima di iniziare

**Usa l'unità il meno possibile.** Un file eliminato resta sull'unità finché non ci viene scritto sopra qualcosa di nuovo. Ogni download, installazione o salvataggio su quell'unità riduce le tue possibilità.

- Se i file erano sull'unità di sistema (di solito `C:`), non installarci nulla. Se puoi, avvia Open Recovr da una chiavetta USB.
- Se erano su una scheda di memoria o una chiavetta USB, toglila e non scattare altre foto con essa.
- **Tieni pronta un'altra unità** per ricevere i file recuperati: una chiavetta USB, un disco esterno o un secondo disco interno.

## 1. Ricerca rapida o approfondita

![La finestra «Fin dove cercare?»](screenshots/en-1-start.png)

All'avvio, Open Recovr chiede fin dove cercare:

- La **ricerca rapida** guarda solo nel Cestino. Richiede pochi secondi e nessun permesso particolare. Se hai svuotato il Cestino, non troverà nulla.
- La **ricerca approfondita** legge l'unità stessa. Richiede i diritti di amministratore: Windows ti chiede di autorizzare l'app (su Linux e macOS ti viene chiesta la password).

Puoi cambiare in seguito con il pulsante **Passa alla ricerca approfondita** in alto. Seleziona *Non propormelo più all'avvio* se scegli sempre la stessa cosa.

## 2. Scegliere l'unità e cosa cerchi

![La schermata iniziale](screenshots/en-2-home.png)

**Dove erano i tuoi file?** Fai clic sull'unità, la scheda o la chiavetta dove si trovavano i file. Ognuna mostra il suo tipo (disco rigido, SSD SATA o NVMe, scheda SD, chiavetta USB, CD…), lo spazio libero e il nome.

**Cerco:** limita la ricerca a *Foto*, *Video*, *Musica*, *Documenti* o *Archivi*. Meno tipi rendono la ricerca più veloce e l'elenco più corto. **Affina** va oltre: formati precisi (solo JPG, solo DOCX…) e una dimensione minima.

Nella ricerca approfondita, le unità NTFS (la maggior parte dei dischi Windows) offrono due analisi:

| Analisi | Cosa legge | Durata | Nomi conservati? |
|---|---|---|---|
| **Rapida** | La tabella dei file dell'unità (MFT) | Pochi minuti | ✅ nome e cartella |
| **Completa** | La tabella dei file, poi tutta l'unità con PhotoRec | Da pochi minuti a oltre un'ora | In parte (vedi sotto) |

Inizia con **Rapida**. Se il tuo file non c'è, fai clic su **Non trovato? Avvia l'analisi completa**.

Su altri file system (schede di memoria FAT32 ed exFAT, ext4, APFS…), la ricerca approfondita passa direttamente all'analisi completa.

## 3. La ricerca

![Una ricerca approfondita in corso](screenshots/en-3-scanning.png)

Fai clic su **Cerca i file persi**. I file compaiono nell'elenco **appena vengono trovati**: non serve aspettare la fine per iniziare a guardare.

Il pannello a destra mostra la fase in corso, l'avanzamento, una stima del tempo rimanente e quanti file sono stati trovati finora.

- **Ferma la ricerca** conserva ciò che è già stato trovato.
- **Continua in background** nasconde la finestra accanto all'orologio (Windows). Fai clic sull'icona per riaprirla. Una notifica ti avvisa quando la ricerca è finita.

Le zone illeggibili, come un CD graffiato o una scheda usurata, vengono saltate. La ricerca prosegue invece di bloccarsi.

## 4. Sfogliare i risultati

![I risultati, con un'anteprima](screenshots/en-4-results.png)

- Le **schede** in alto (*Tutto*, *Foto*, *Documenti*…) indicano quanti file di ogni tipo sono stati trovati.
- **Formato** e **Dimensione** filtrano ulteriormente. La casella di ricerca filtra per nome.
- Fai clic sul titolo di una colonna per **ordinare** per tipo, nome, data o dimensione.
- Fai clic su un file per vederne l'**anteprima** a destra, con i dettagli. Si possono visualizzare foto, video, suoni, PDF, testi e documenti Word. **Apri** lo apre nel suo programma abituale, da una copia temporanea.

Ogni file trovato viene letto dall'inizio alla fine, e la colonna **Stato** dà il risultato:

| Stato | Significato |
|---|---|
| **Integro** | Il file è stato controllato dall'inizio alla fine ed è completo. Tornerà com'era. |
| **Danneggiato** | Una parte manca o è errata. Il dettaglio dice perché: troncato, mescolato con un altro file, indice mancante… Potrebbe ancora aprirsi in parte. |
| **Vuoto** | Lo spazio che occupava contiene solo zeri. Non c'è più nulla da recuperare. |

Lo stesso file viene spesso trovato più volte (nel Cestino e sull'unità, per esempio). Le copie sono nascoste dietro un link *duplicati nascosti · Mostra*.

I file trovati da PhotoRec non hanno un nome proprio. Open Recovr gliene dà uno in base al contenuto quando può: `photo_2024-07-14_18-22-05.jpg` dalla data di scatto di una foto, il titolo di un documento Word o PDF, l'artista e il brano di un MP3.

## 5. Selezionare ciò che vuoi recuperare

![Foto selezionate per il recupero](screenshots/en-5-selected.png)

Seleziona i file uno per uno, oppure usa la casella nell'intestazione della colonna per selezionare tutto nella vista corrente. **Seleziona tutto** e **Deseleziona tutto**, in basso, agiscono sull'intero elenco. La barra in basso indica quanti file sono selezionati e la loro dimensione totale.

## 6. Recuperare

![Recupero completato](screenshots/en-6-recovered.png)

Fai clic su **Recupera la selezione** e scegli una cartella **su un'altra unità**. Se scegli l'unità analizzata, Open Recovr ti avvisa che potresti sovrascrivere altri file recuperabili.

I file vengono copiati in una nuova cartella chiamata *File recuperati* seguita dalla data, con una sottocartella per tipo (Foto, Documenti…). Una cartella spostata nel Cestino torna intera. Alla fine, **Apri la cartella** te li mostra.

Sull'unità analizzata non viene mai scritto nulla. I file vengono solo letti.

## Immagini disco

Un'unità che si sta guastando (fa clic, è molto lenta, piena di errori di lettura) può cedere del tutto durante la ricerca. È più sicuro copiarla una volta, settore per settore, con uno strumento come [ddrescue](https://www.gnu.org/software/ddrescue/), e poi analizzare la copia.

La scheda **Immagine disco**, accanto alle unità, apre una copia di questo tipo (`.img`, `.dd`, `.raw`…). La ricerca funziona allora come su un'unità, senza diritti di amministratore, e l'originale non viene toccato.

## Impostazioni, temi e aggiornamenti

![La pagina delle impostazioni](screenshots/en-8-settings.png)

Il menu in alto a destra (☰) raccoglie tutto ciò che non serve direttamente a recuperare file:

- **Impostazioni**: la lingua, il tema dei colori, la modalità esperto, se proporre la ricerca approfondita all'avvio, gli aggiornamenti e le informazioni sull'app.
- **Tema dei colori**: chiaro, scuro, Nord, Catppuccin, Tokyo Night, Solarized, Monokai o Gruvbox. Il pulsante sole/luna, accanto alla lingua, passa da chiaro a scuro con un clic.
- **Cerca aggiornamenti**, **Prepara un rapporto…** e la pagina del progetto su GitHub.

Quando esce una nuova versione, la barra in basso mostra **→ v…: aggiorna**. Un clic la scarica, verifica che sia firmata con la chiave del progetto, la installa e riavvia Open Recovr. Se è in corso una ricerca o un recupero, l'aggiornamento aspetta che finisca.

## Modalità esperto

![La modalità esperto con il registro della ricerca](screenshots/en-7-expert.png)

La **Modalità esperto**, nel menu in alto a destra, mostra il registro della ricerca sotto l'elenco: l'unità e il suo file system, ogni fase e le righe di PhotoRec. **Copia tutto** copia il registro, utile per segnalare un problema. Trascina il bordo superiore del registro per ingrandirlo.

Ogni ricerca viene anche scritta in `journal.txt`, accanto alle impostazioni (`%APPDATA%\Open Recovr` su Windows). Il link **Registro**, in basso, lo apre.

## Chiedere aiuto a qualcuno

Nel menu in alto a destra (☰), **Prepara un rapporto…** salva uno zip da inviare a chi ti aiuta, per e-mail o in altro modo. Open Recovr si limita a scrivere il file: non lo invia mai. Contiene file di testo: cos'è il rapporto, il sistema e le unità (modello, dimensione, file system), le impostazioni e il registro dell'ultima ricerca. Prima di scrivere qualsiasi cosa, il nome del tuo account, il nome del computer, la tua cartella personale e gli indirizzi e-mail vengono mascherati, come pure i titoli dei documenti che PhotoRec mette nei nomi dei file che trova. Apri lo zip e rileggilo prima di inoltrarlo.

## Domande frequenti

**Il mio file non è nell'elenco. Cosa faccio?**
Verifica che sia scelta l'unità giusta e che il filtro per tipo non lo nasconda (*Tutto*). Poi prova la ricerca approfondita e, su NTFS, l'analisi completa. Se l'unità è stata usata molto dopo l'eliminazione, il file potrebbe essere stato sovrascritto. Allora nessun programma può riportarlo indietro.

**I file recuperati si chiamano `f1234567.jpg`.**
Questi file sono stati trovati da PhotoRec, che riconosce un file dal suo contenuto e non dalla tabella dei file: il loro nome originale è sconosciuto. Quando il file stesso non dice nulla della sua data o del suo titolo, il numero resta. Ordina per data o dimensione e usa l'anteprima per trovare il tuo.

**Un file è segnato come Danneggiato. Vale la pena recuperarlo?**
Spesso sì. Una foto danneggiata può mostrare solo la metà superiore, un video danneggiato può essere riprodotto fino al punto rovinato. Recuperalo e provalo: non costa nulla, e l'unità non viene mai modificata.

**Può recuperare i file del mio telefono?**
Non via USB. Un telefono condivide solo i file che ha ancora, e la sua memoria è cifrata. Guarda nel cestino dell'app Galleria o Foto (le foto eliminate restano di solito 30 giorni), in Google Foto o iCloud, oppure inserisci la scheda microSD del telefono in un lettore di schede e analizzala.

**Funziona su un SSD?**
In parte. La maggior parte degli SSD cancella in background i blocchi eliminati (TRIM): i file eliminati da tempo sono spesso persi per sempre. Il Cestino e la tabella dei file aiutano ancora per le eliminazioni recenti. Agisci in fretta.

**Si possono recuperare file da un'unità formattata?**
Spesso sì, con l'analisi completa. Una formattazione rapida lascia il contenuto al suo posto. Una formattazione completa (o «cancellazione sicura») lo sovrascrive.

**Perché la ricerca approfondita richiede i diritti di amministratore?**
Legge l'unità settore per settore, al di sotto del file system, e il sistema operativo lo riserva agli amministratori. Open Recovr legge soltanto: non scrive mai su quell'unità.

**È sicuro? Invia i miei dati da qualche parte?**
Open Recovr funziona offline. Nessun account, nessuna telemetria. La sua unica connessione è la ricerca di aggiornamenti: all'avvio chiede a GitHub il numero dell'ultima versione, e non invia nient'altro. Deseleziona «Cerca aggiornamenti all'avvio» nelle Impostazioni per disattivarla; il menu può sempre controllare su richiesta. Un aggiornamento viene installato solo se è firmato con la chiave del progetto, che non va mai su GitHub: una versione falsa caricata da qualcun altro viene rifiutata. Il codice è aperto: verifica tu stesso.

**Linux / macOS: la ricerca approfondita dice che PhotoRec non è disponibile.**
Installalo: `sudo apt install testdisk` (Debian, Ubuntu), `sudo dnf install testdisk` (Fedora) o `brew install testdisk` (macOS). Open Recovr trova allora `photorec` da solo. Senza di esso, un motore integrato più semplice cerca i formati più comuni.
