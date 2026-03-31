# Modulo S: Configurazione Sovrana

**Corso STREETS per il Reddito degli Sviluppatori — Modulo Gratuito**
*Settimane 1-2 | 6 Lezioni | Deliverable: Il Tuo Documento dello Stack Sovrano*

> "Il tuo rig è la tua infrastruttura aziendale. Configuralo come tale."

---

Possiedi già lo strumento più potente per generare reddito che la maggior parte delle persone non avrà mai: una workstation da sviluppatore con connessione internet, calcolo locale e le competenze per collegare tutto insieme.

La maggior parte degli sviluppatori tratta il proprio rig come un prodotto di consumo. Qualcosa su cui giocano, programmano, navigano. Ma quella stessa macchina — quella che hai sotto la scrivania in questo momento — può eseguire inferenza, servire API, elaborare dati e generare entrate 24 ore al giorno mentre dormi.

Questo modulo riguarda il guardare ciò che già hai attraverso una lente diversa. Non "cosa posso costruire?" ma "cosa posso vendere?"

Alla fine di queste due settimane, avrai:

- Un inventario chiaro delle tue capacità di generazione di reddito
- Uno stack LLM locale di livello produzione
- Una base legale e finanziaria (anche se minima)
- Un Documento dello Stack Sovrano scritto che diventerà il tuo piano aziendale

Niente giri di parole. Niente "credi in te stesso." Numeri reali, comandi reali, decisioni reali.

{@ mirror sovereign_readiness @}

Cominciamo.

---

## Lezione 1: L'Audit del Rig

*"Non ti serve una 4090. Ecco cosa conta davvero."*

### La Tua Macchina È un Asset Aziendale

Quando un'azienda valuta la propria infrastruttura, non si limita a elencare le specifiche — mappa le capacità alle opportunità di guadagno. È esattamente quello che farai adesso.

{? if computed.profile_completeness != "0" ?}
> **Il Tuo Rig Attuale:** {= profile.cpu.model | fallback("CPU sconosciuta") =} ({= profile.cpu.cores | fallback("?") =} core / {= profile.cpu.threads | fallback("?") =} thread), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("Nessuna GPU dedicata") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} liberi / {= profile.storage.total | fallback("?") =} totali ({= profile.storage.type | fallback("sconosciuto") =}), con {= profile.os.name | fallback("OS sconosciuto") =} {= profile.os.version | fallback("") =}.
{? endif ?}

Apri un terminale e segui i passaggi seguenti. Scrivi ogni numero. Ti serviranno per il Documento dello Stack Sovrano nella Lezione 6.

### Inventario Hardware

#### CPU

```bash
# Linux/Mac
lscpu | grep "Model name\|CPU(s)\|Thread(s)"
# oppure
cat /proc/cpuinfo | grep "model name" | head -1
nproc

# Windows (PowerShell)
Get-CimInstance -ClassName Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors

# macOS
sysctl -n machdep.cpu.brand_string
sysctl -n hw.ncpu
```

**Cosa conta per il reddito:**
- Il numero di core determina quanti task concorrenti il tuo rig può gestire. Eseguire un LLM locale mentre contemporaneamente elabori un job in batch richiede vero parallelismo.
{? if profile.cpu.cores ?}
- *Il tuo {= profile.cpu.model | fallback("CPU") =} ha {= profile.cpu.cores | fallback("?") =} core — controlla la tabella dei requisiti qui sotto per vedere quali motori di guadagno supporta la tua CPU.*
{? endif ?}
- Per la maggior parte dei motori di guadagno in questo corso, qualsiasi CPU moderna 8+ core degli ultimi 5 anni è sufficiente.
- Se esegui LLM locali solo su CPU (senza GPU), ti servono 16+ core. Un Ryzen 7 5800X o Intel i7-12700 è il minimo pratico.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**Cosa conta per il reddito:**
- 16 GB: Minimo indispensabile. Puoi eseguire modelli 7B e fare lavoro di automazione base.
- 32 GB: Confortevole. Esegui modelli 13B localmente, gestisci più progetti, mantieni il tuo ambiente di sviluppo attivo insieme ai carichi di lavoro produttivi.
- 64 GB+: Puoi eseguire modelli 30B+ su CPU, o tenere più modelli caricati. Qui le cose diventano interessanti per vendere servizi di inferenza.
{? if profile.ram.total ?}
*Il tuo sistema ha {= profile.ram.total | fallback("?") =} di RAM. Controlla la tabella sopra per vedere in quale livello di capacità ti trovi — questo influenza direttamente quali modelli locali sono pratici per i tuoi carichi di lavoro produttivi.*
{? endif ?}

#### GPU

```bash
# NVIDIA
nvidia-smi

# Controllare specificamente la VRAM
nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv

# AMD (Linux)
rocm-smi

# macOS (Apple Silicon)
system_profiler SPDisplaysDataType
```

**Cosa conta per il reddito:**

Questa è l'unica specifica su cui le persone sono ossessionate, e qui c'è la verità onesta: **la tua GPU determina il tuo livello di LLM locale, e il tuo livello di LLM locale determina quali flussi di reddito funzionano più velocemente.** Ma non determina se puoi guadagnare o no.

| VRAM | Capacità LLM | Rilevanza per il Reddito |
|------|---------------|--------------------------|
| 0 (solo CPU) | Modelli 7B a ~5 token/sec | Elaborazione batch, lavoro asincrono. Lento ma funzionale. |
| 6-8 GB (RTX 3060, ecc.) | Modelli 7B a ~30 tok/sec, 13B quantizzato | Sufficiente per la maggior parte dei flussi di reddito da automazione. |
| 12 GB (RTX 3060 12GB, 4070) | 13B a piena velocità, 30B quantizzato | Punto ottimale. La maggior parte dei motori di guadagno funziona bene qui. |
| 16-24 GB (RTX 4090, 3090) | Modelli 30B-70B | Livello premium. Vendi qualità che altri non possono eguagliare localmente. |
| 48 GB+ (doppia GPU, A6000) | 70B+ a velocità | Inferenza locale di livello enterprise. Serio vantaggio competitivo. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ usando memoria unificata | Eccellente efficienza. Costo energetico inferiore rispetto all'equivalente NVIDIA. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **La Tua GPU:** {= profile.gpu.model | fallback("Sconosciuta") =} con {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}sei nel livello premium. Modelli 30B-70B sono raggiungibili localmente. Questo è un serio vantaggio competitivo.{? elif computed.gpu_tier == "sweet_spot" ?}sei nel punto ottimale. 13B a piena velocità, 30B quantizzato. La maggior parte dei motori di guadagno funziona bene qui.{? elif computed.gpu_tier == "capable" ?}puoi eseguire modelli 7B a buona velocità e 13B quantizzato. Sufficiente per la maggior parte dei flussi di reddito da automazione.{? else ?}hai accelerazione GPU disponibile. Controlla la tabella sopra per vedere dove ti posizioni.{? endif ?}
{? else ?}
> **Nessuna GPU dedicata rilevata.** Eseguirai l'inferenza su CPU, il che significa ~5-12 token/sec su modelli 7B. Va bene per elaborazione batch e lavoro asincrono. Usa chiamate API per colmare il gap di velocità per output rivolti ai clienti.
{? endif ?}

> **Parliamoci Chiaro:** Se hai una RTX 3060 12GB, sei in una posizione migliore del 95% degli sviluppatori che cercano di monetizzare l'IA. Smetti di aspettare una 4090. La 3060 12GB è la Honda Civic dell'IA locale — affidabile, efficiente, fa il suo lavoro. I soldi che spenderesti per un upgrade della GPU sono meglio spesi in crediti API per qualità rivolta ai clienti mentre i tuoi modelli locali gestiscono il lavoro pesante.

#### Storage

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**Cosa conta per il reddito:**
- I modelli LLM occupano spazio: modello 7B = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (quantizzato).
- Ti serve spazio per dati di progetto, database, cache e artefatti di output.
- L'SSD è imprescindibile per qualsiasi cosa rivolta ai clienti. Il caricamento dei modelli da HDD aggiunge 30-60 secondi di tempo di avvio.
- Minimo pratico: 500 GB SSD con almeno 100 GB liberi.
- Confortevole: 1 TB SSD. Tieni i modelli sull'SSD, archivia su HDD.
{? if profile.storage.free ?}
*Hai {= profile.storage.free | fallback("?") =} liberi su {= profile.storage.type | fallback("il tuo drive") =}. {? if profile.storage.type == "SSD" ?}Bene — SSD significa caricamento rapido dei modelli.{? elif profile.storage.type == "NVMe" ?}Eccellente — NVMe è l'opzione più veloce per il caricamento dei modelli.{? else ?}Considera un SSD se non ne hai già uno — fa una vera differenza per i tempi di caricamento dei modelli.{? endif ?}*
{? endif ?}

#### Rete

```bash
# Speed test rapido (installa speedtest-cli se necessario)
# pip install speedtest-cli
speedtest-cli --simple

# Oppure controlla semplicemente il tuo piano
# La velocità di upload conta più del download per servire contenuti
```

**Cosa conta per il reddito:**
{? if profile.network.download ?}
*La tua connessione: {= profile.network.download | fallback("?") =} down / {= profile.network.upload | fallback("?") =} up.*
{? endif ?}
- Velocità di download: 50+ Mbps. Necessaria per scaricare modelli, pacchetti e dati.
- Velocità di upload: Questo è il collo di bottiglia che la maggior parte delle persone ignora. Se stai servendo qualcosa (API, risultati elaborati, deliverable), l'upload conta.
  - 10 Mbps: Adeguato per consegna asincrona (file elaborati, risultati batch).
  - 50+ Mbps: Necessario se stai eseguendo qualsiasi tipo di endpoint API locale che servizi esterni interrogano.
  - 100+ Mbps: Confortevole per tutto in questo corso.
- Latenza: Sotto 50ms verso i principali provider cloud. Esegui `ping api.openai.com` e `ping api.anthropic.com` per verificare.

#### Uptime

Questa è la specifica a cui nessuno pensa, ma che separa gli hobbisti da chi guadagna mentre dorme.

Chiediti:
- Il tuo rig può funzionare 24/7? (Alimentazione, raffreddamento, rumore)
- Hai un UPS per le interruzioni di corrente?
- La tua connessione internet è abbastanza stabile per workflow automatizzati?
- Puoi accedere via SSH alla tua macchina da remoto se qualcosa si rompe?

Se non puoi funzionare 24/7, va bene — molti flussi di reddito in questo corso sono job batch asincroni che avvii manualmente. Ma quelli che generano reddito veramente passivo richiedono uptime.

{? if computed.os_family == "windows" ?}
**Setup rapido uptime (Windows):** Usa l'Utilità di pianificazione per il riavvio automatico, abilita Desktop Remoto o installa Tailscale per l'accesso remoto, e configura il BIOS per "ripristino all'interruzione dell'alimentazione CA" per recuperare dai blackout.
{? endif ?}

**Setup rapido uptime (se lo desideri):**

```bash
# Abilita Wake-on-LAN (controlla il BIOS)
# Configura l'accesso SSH
sudo systemctl enable ssh  # Linux

# Auto-riavvio su crash (esempio servizio systemd)
# /etc/systemd/system/my-income-worker.service
[Unit]
Description=Income Worker Process
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/my-worker
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

### La Matematica dell'Elettricità

Le persone o ignorano questo aspetto o lo drammatizzano. Facciamo vera matematica.

**Misurare il tuo consumo reale:**

```bash
# Se hai un misuratore Kill-A-Watt o una presa smart con monitoraggio:
# Misura a riposo, sotto carico (inferenza in esecuzione) e al massimo (GPU a pieno utilizzo)

# Stime approssimative se non hai un misuratore:
# Desktop (senza GPU, a riposo): 60-100W
# Desktop (GPU di fascia media, a riposo): 80-130W
# Desktop (GPU di fascia alta, a riposo): 100-180W
# Desktop (GPU sotto carico inferenza): aggiungi il 50-80% del TDP della GPU
# Laptop: 15-45W
# Mac Mini M2: 7-15W (sul serio)
# Laptop Apple Silicon: 10-30W
```

**Calcolo del costo mensile:**

```
Costo mensile = (Watt / 1000) x Ore x Prezzo per kWh

Esempio: Desktop con RTX 3060, inferenza in esecuzione 8 ore/giorno, a riposo 16 ore/giorno
- Inferenza: (250W / 1000) x 8h x 30 giorni x $0.12/kWh = $7.20/mese
- Riposo: (100W / 1000) x 16h x 30 giorni x $0.12/kWh = $5.76/mese
- Totale: ~$13/mese

Esempio: Stesso rig, inferenza 24/7
- (250W / 1000) x 24h x 30 giorni x $0.12/kWh = $21.60/mese

Esempio: Mac Mini M2, 24/7
- (12W / 1000) x 24h x 30 giorni x $0.12/kWh = $1.04/mese
```

{? if regional.country ?}
La tua tariffa elettrica: circa {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh (basato sulle medie di {= regional.country | fallback("la tua regione") =}). Controlla la tua bolletta effettiva — le tariffe variano per fornitore e fascia oraria.
{? else ?}
La media USA dell'elettricità è circa $0.12/kWh. Controlla la tua tariffa reale — varia enormemente. La California potrebbe essere $0.25/kWh. Alcuni paesi europei arrivano a $0.35/kWh. Alcune zone del Midwest USA sono a $0.08/kWh.
{? endif ?}

**Il punto:** Far funzionare il tuo rig 24/7 per generare reddito costa da qualche parte tra {= regional.currency_symbol | fallback("$") =}1 e {= regional.currency_symbol | fallback("$") =}30/mese in elettricità. Se i tuoi flussi di reddito non coprono quello, il problema non è l'elettricità — è il flusso di reddito.

### Specifiche Minime per Tipo di Motore di Guadagno

Ecco un'anteprima di dove stiamo andando nel corso STREETS completo. Per ora, controlla semplicemente dove si posiziona il tuo rig:

| Motore di Guadagno | CPU | RAM | GPU | Storage | Rete |
|---------------------|-----|-----|-----|---------|------|
| **Automazione contenuti** (post blog, newsletter) | 4+ core | 16 GB | Opzionale (fallback API) | 50 GB liberi | 10 Mbps up |
| **Servizi di elaborazione dati** | 8+ core | 32 GB | Opzionale | 200 GB liberi | 50 Mbps up |
| **Servizi API di IA locale** | 8+ core | 32 GB | 8+ GB VRAM | 100 GB liberi | 50 Mbps up |
| **Strumenti di generazione codice** | 8+ core | 16 GB | 8+ GB VRAM o API | 50 GB liberi | 10 Mbps up |
| **Elaborazione documenti** | 4+ core | 16 GB | Opzionale | 100 GB liberi | 10 Mbps up |
| **Agenti autonomi** | 8+ core | 32 GB | 12+ GB VRAM | 100 GB liberi | 50 Mbps up |

> **Errore Comune:** "Devo aggiornare il mio hardware prima di poter iniziare." No. Inizia con quello che hai. Usa chiamate API per colmare i gap che il tuo hardware non copre. Aggiorna quando il guadagno lo giustifica — non prima.

{@ insight engine_ranking @}

### Checkpoint Lezione 1

Dovresti ora aver annotato:
- [ ] Modello CPU, core e thread
- [ ] Quantità di RAM
- [ ] Modello GPU e VRAM (o "nessuna")
- [ ] Storage disponibile
- [ ] Velocità di rete (down/up)
- [ ] Costo mensile stimato dell'elettricità per operatività 24/7
- [ ] Per quali categorie di motori di guadagno il tuo rig è qualificato

Conserva questi numeri. Li inserirai nel tuo Documento dello Stack Sovrano nella Lezione 6.

{? if computed.profile_completeness != "0" ?}
> **4DA ha già raccolto la maggior parte di questi numeri per te.** Controlla i riepiloghi personalizzati sopra — il tuo inventario hardware è parzialmente pre-compilato dal rilevamento del sistema.
{? endif ?}

*Nel corso STREETS completo, il Modulo R (Motori di Guadagno) ti dà playbook specifici, passo dopo passo, per ogni tipo di motore elencato sopra — incluso il codice esatto per costruirli e distribuirli.*

---

## Lezione 2: Lo Stack LLM Locale

*"Configura Ollama per uso in produzione — non solo per chattare."*

### Perché gli LLM Locali Contano per il Reddito

Ogni volta che chiami l'API di OpenAI, stai pagando un affitto. Ogni volta che esegui un modello localmente, quell'inferenza è gratuita dopo la configurazione iniziale. La matematica è semplice:

- GPT-4o: ~$5 per milione di token in input, ~$15 per milione di token in output
- Claude 3.5 Sonnet: ~$3 per milione di token in input, ~$15 per milione di token in output
- Llama 3.1 8B locale: $0 per milione di token (solo elettricità)

Se stai costruendo servizi che elaborano migliaia di richieste, la differenza tra $0 e $5-$15 per milione di token è la differenza tra profitto e pareggio.

Ma ecco la sfumatura che la maggior parte delle persone non coglie: **i modelli locali e le API servono ruoli diversi in uno stack di reddito.** I modelli locali gestiscono il volume. Le API gestiscono l'output di qualità critica, rivolto ai clienti. Il tuo stack ha bisogno di entrambi.

### Installare Ollama

{? if settings.has_llm ?}
> **Hai già un LLM configurato:** {= settings.llm_provider | fallback("Locale") =} / {= settings.llm_model | fallback("modello sconosciuto") =}. Se Ollama è già in esecuzione, salta alla "Guida alla Selezione dei Modelli" sotto.
{? endif ?}

Ollama è la base. Trasforma la tua macchina in un server di inferenza locale con un'API pulita.

```bash
# Linux
curl -fsSL https://ollama.com/install.sh | sh

# macOS
# Scarica da https://ollama.com oppure:
brew install ollama

# Windows
# Scarica l'installer da https://ollama.com
# Oppure usa winget:
winget install Ollama.Ollama
```

{? if computed.os_family == "windows" ?}
> **Windows:** Usa l'installer da ollama.com o `winget install Ollama.Ollama`. Ollama si esegue come servizio in background automaticamente dopo l'installazione.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` è il percorso più veloce. Ollama sfrutta la memoria unificata di Apple Silicon — i tuoi {= profile.ram.total | fallback("sistema") =} di RAM sono condivisi tra carichi di lavoro CPU e GPU.
{? elif computed.os_family == "linux" ?}
> **Linux:** Lo script di installazione gestisce tutto. Se stai eseguendo {= profile.os.name | fallback("Linux") =}, Ollama si installa come servizio systemd.
{? endif ?}

Verifica l'installazione:

```bash
ollama --version
# Dovrebbe mostrare versione 0.5.x o superiore (controlla https://ollama.com/download per l'ultima versione)

# Avvia il server (se non si avvia automaticamente)
ollama serve

# In un altro terminale, testalo:
ollama run llama3.1:8b "Salutami in esattamente 5 parole"
```

> **Nota sulla versione:** Ollama rilascia aggiornamenti frequentemente. I comandi modello e i flag in questo modulo sono stati verificati con Ollama v0.5.x (inizio 2026). Se stai leggendo questo più tardi, controlla [ollama.com/download](https://ollama.com/download) per l'ultima versione e [ollama.com/library](https://ollama.com/library) per i nomi dei modelli attuali. I concetti fondamentali non cambiano, ma tag specifici dei modelli (es. `llama3.1:8b`) potrebbero essere superati da versioni più recenti.

### Guida alla Selezione dei Modelli

Non scaricare ogni modello che vedi. Sii strategico. Ecco cosa scaricare e quando usare ciascuno.

{? if computed.llm_tier ?}
> **Il tuo livello LLM (basato sull'hardware):** {= computed.llm_tier | fallback("sconosciuto") =}. Le raccomandazioni sotto sono etichettate così puoi concentrarti sul livello che corrisponde al tuo rig.
{? endif ?}

#### Livello 1: Il Cavallo di Battaglia (modelli 7B-8B)

```bash
# Scarica il tuo modello cavallo di battaglia
ollama pull llama3.1:8b
# Alternativa: mistral (buono per lingue europee)
ollama pull mistral:7b
```

**Usa per:**
- Classificazione testo ("Questa email è spam o legittima?")
- Riassunti (condensare documenti lunghi in punti elenco)
- Estrazione dati semplice (estrarre nomi, date, importi dal testo)
- Analisi del sentiment
- Tagging e categorizzazione contenuti
- Generazione embedding (se usi un modello con supporto embedding)

**Performance (tipiche):**
- RTX 3060 12GB: ~40-60 token/secondo
- RTX 4090: ~100-130 token/secondo
- M2 Pro 16GB: ~30-45 token/secondo
- Solo CPU (Ryzen 7 5800X): ~8-12 token/secondo

**Confronto costi:**
- 1 milione di token via GPT-4o-mini: ~$0.60
- 1 milione di token localmente (modello 8B): ~$0.003 in elettricità
- Punto di pareggio: ~5.000 token (risparmi dal letteralmente primo richiesta)

#### Livello 2: La Scelta Bilanciata (modelli 13B-14B)

```bash
# Scarica il tuo modello bilanciato
ollama pull llama3.1:14b
# Oppure per task di coding:
ollama pull deepseek-coder-v2:16b
```

**Usa per:**
- Redazione contenuti (post del blog, documentazione, testi di marketing)
- Generazione codice (funzioni, script, boilerplate)
- Trasformazione dati complessa
- Task di ragionamento multi-step
- Traduzione con sfumature

**Performance (tipiche):**
- RTX 3060 12GB: ~20-30 token/secondo (quantizzato)
- RTX 4090: ~60-80 token/secondo
- M2 Pro 32GB: ~20-30 token/secondo
- Solo CPU: ~3-6 token/secondo (non pratico per tempo reale)

**Quando usare rispetto al 7B:** Quando la qualità dell'output del 7B non è abbastanza buona ma non devi pagare per chiamate API. Testa entrambi sul tuo caso d'uso reale — a volte il 7B va bene e stai solo sprecando calcolo.

{? if computed.gpu_tier == "capable" ?}
> **Territorio del Livello 3** — Il tuo {= profile.gpu.model | fallback("GPU") =} può gestire il 30B quantizzato con un po' di sforzo, ma il 70B è fuori portata localmente. Considera chiamate API per task che necessitano di qualità a livello 70B.
{? endif ?}

#### Livello 3: Il Livello Qualità (modelli 30B-70B)

```bash
# Scarica questi solo se hai la VRAM necessaria
# 30B richiede ~20GB VRAM, 70B richiede ~40GB VRAM (quantizzato)
ollama pull llama3.1:70b-instruct-q4_K_M
# Oppure l'eccellente ma più piccolo:
ollama pull qwen2.5:32b
```

**Usa per:**
- Contenuti rivolti ai clienti che devono essere eccellenti
- Analisi e ragionamento complessi
- Generazione di contenuti lunghi
- Task dove la qualità impatta direttamente se qualcuno ti paga

**Performance (tipiche):**
- RTX 4090 (24GB): 70B a ~8-15 token/secondo (usabile ma lento)
- Doppia GPU o 48GB+: 70B a ~20-30 token/secondo
- M3 Max 64GB: 70B a ~10-15 token/secondo

> **Parliamoci Chiaro:** Se non hai 24GB+ di VRAM, salta completamente i modelli 70B. Usa chiamate API per output di qualità critica. Un modello 70B che gira a 3 token/secondo dalla RAM di sistema è tecnicamente possibile ma praticamente inutile per qualsiasi workflow che generi reddito. Il tuo tempo ha valore.

#### Livello 4: Modelli API (Quando il Locale Non Basta)

I modelli locali sono per volume e privacy. I modelli API sono per tetti di qualità e capacità specializzate.

**Quando usare modelli API:**
- Output rivolto ai clienti dove qualità = guadagno (testi di vendita, contenuti premium)
- Catene di ragionamento complesse su cui i modelli più piccoli sbagliano
- Task di visione/multimodali (analisi di immagini, screenshot, documenti)
- Quando ti serve output JSON strutturato con alta affidabilità
- Quando la velocità conta e il tuo hardware locale è lento

**Tabella di confronto costi (inizio 2025 — controlla i prezzi attuali):**

| Modello | Input (per 1M token) | Output (per 1M token) | Migliore Per |
|---------|----------------------|-----------------------|--------------|
| GPT-4o-mini | $0.15 | $0.60 | Lavoro di volume economico (quando il locale non è disponibile) |
| GPT-4o | $2.50 | $10.00 | Visione, ragionamento complesso |
| Claude 3.5 Sonnet | $3.00 | $15.00 | Codice, analisi, contesto lungo |
| Claude 3.5 Haiku | $0.80 | $4.00 | Veloce, economico, buon equilibrio qualità |
| DeepSeek V3 | $0.27 | $1.10 | Economico, performance solide |

**La strategia ibrida:**
1. LLM locale 7B/13B gestisce l'80% delle richieste (classificazione, estrazione, riassunti)
2. API gestisce il 20% delle richieste (passaggio di qualità finale, task complessi, output rivolto ai clienti)
3. Il tuo costo effettivo: ~$0.50-2.00 per milione di token misto (invece di $5-15 solo API)

Questo approccio ibrido è come costruisci servizi con margini sani. Approfondiremo nel Modulo R.

### Configurazione di Produzione

Eseguire Ollama per lavoro produttivo è diverso dall'eseguirlo per chat personale. Ecco come configurarlo correttamente.

{? if computed.has_nvidia ?}
> **GPU NVIDIA rilevata ({= profile.gpu.model | fallback("sconosciuta") =}).** Ollama utilizzerà automaticamente l'accelerazione CUDA. Assicurati che i driver NVIDIA siano aggiornati — esegui `nvidia-smi` per verificare. Per le migliori performance con {= profile.gpu.vram | fallback("la tua") =} VRAM, l'impostazione `OLLAMA_MAX_LOADED_MODELS` sotto dovrebbe corrispondere a quanti modelli possono stare nella tua VRAM contemporaneamente.
{? endif ?}

#### Imposta le Variabili d'Ambiente

```bash
# Crea/modifica la configurazione di Ollama
# Linux: /etc/systemd/system/ollama.service o variabili d'ambiente
# macOS: ambiente launchctl o ~/.zshrc
# Windows: Variabili d'Ambiente di Sistema

# Impostazioni chiave:
export OLLAMA_HOST=127.0.0.1:11434    # Bind solo a localhost (sicurezza)
export OLLAMA_NUM_PARALLEL=4            # Gestione richieste concorrenti
export OLLAMA_MAX_LOADED_MODELS=2       # Mantieni 2 modelli in memoria
export OLLAMA_KEEP_ALIVE=30m            # Tieni il modello caricato per 30 min dopo l'ultima richiesta
export OLLAMA_MAX_QUEUE=100             # Accoda fino a 100 richieste
```

#### Crea un Modelfile per il Tuo Carico di Lavoro

Invece di usare le impostazioni di default del modello, crea un Modelfile personalizzato calibrato per il tuo carico di lavoro produttivo:

```dockerfile
# Salva come: Modelfile-worker
FROM llama3.1:8b

# Calibrato per output di produzione consistente
PARAMETER temperature 0.3
PARAMETER top_p 0.9
PARAMETER num_ctx 4096
PARAMETER repeat_penalty 1.1

# Prompt di sistema per il tuo carico di lavoro più comune
SYSTEM """You are a precise data processing assistant. You follow instructions exactly. You output only what is requested, with no preamble or explanation unless asked. When given structured output formats (JSON, CSV, etc.), you output only the structure with no markdown formatting."""
```

```bash
# Crea il tuo modello personalizzato
ollama create worker -f Modelfile-worker

# Testalo
ollama run worker "Extract all email addresses from this text: Contact us at hello@example.com or support@test.org for more info."
```

#### Batching e Gestione delle Code

Per carichi di lavoro produttivi, spesso dovrai elaborare molti elementi. Ecco un setup base di batching:

```python
#!/usr/bin/env python3
"""
batch_processor.py — Elabora elementi attraverso LLM locale con accodamento.
Batching di livello produzione per carichi di lavoro redditizi.
"""

import requests
import json
import time
import concurrent.futures
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "worker"  # Il tuo modello personalizzato dal precedente
MAX_CONCURRENT = 4
MAX_RETRIES = 3

def process_item(item: dict) -> dict:
    """Elabora un singolo elemento attraverso l'LLM locale."""
    payload = {
        "model": MODEL,
        "prompt": item["prompt"],
        "stream": False,
        "options": {
            "num_ctx": 4096,
            "temperature": 0.3
        }
    }

    for attempt in range(MAX_RETRIES):
        try:
            response = requests.post(OLLAMA_URL, json=payload, timeout=120)
            response.raise_for_status()
            result = response.json()
            return {
                "id": item["id"],
                "input": item["prompt"][:100],
                "output": result["response"],
                "tokens": result.get("eval_count", 0),
                "duration_ms": result.get("total_duration", 0) / 1_000_000,
                "status": "success"
            }
        except Exception as e:
            if attempt == MAX_RETRIES - 1:
                return {
                    "id": item["id"],
                    "output": None,
                    "error": str(e),
                    "status": "failed"
                }
            time.sleep(2 ** attempt)  # Backoff esponenziale

def process_batch(items: list[dict], output_file: str = "results.jsonl"):
    """Elabora un batch di elementi con esecuzione concorrente."""
    results = []
    start_time = time.time()

    with concurrent.futures.ThreadPoolExecutor(max_workers=MAX_CONCURRENT) as executor:
        future_to_item = {executor.submit(process_item, item): item for item in items}

        for i, future in enumerate(concurrent.futures.as_completed(future_to_item)):
            result = future.result()
            results.append(result)

            # Scrittura incrementale (non perdere i progressi in caso di crash)
            with open(output_file, "a") as f:
                f.write(json.dumps(result) + "\n")

            # Report dei progressi
            elapsed = time.time() - start_time
            rate = (i + 1) / elapsed
            remaining = (len(items) - i - 1) / rate if rate > 0 else 0
            print(f"[{i+1}/{len(items)}] {result['status']} | "
                  f"{rate:.1f} elementi/sec | "
                  f"ETA: {remaining:.0f}s")

    # Riepilogo
    succeeded = sum(1 for r in results if r["status"] == "success")
    failed = sum(1 for r in results if r["status"] == "failed")
    total_time = time.time() - start_time

    print(f"\nBatch completato: {succeeded} riusciti, {failed} falliti, "
          f"{total_time:.1f}s totali")

    return results

# Esempio di utilizzo:
if __name__ == "__main__":
    # I tuoi elementi da elaborare
    items = [
        {"id": i, "prompt": f"Summarize this in one sentence: {text}"}
        for i, text in enumerate(load_your_data())  # Sostituisci con la tua fonte dati
    ]

    results = process_batch(items)
```

### Benchmarking del TUO Rig

Non fidarti dei benchmark di altri. Misura i tuoi:

```bash
# Script di benchmark rapido
# Salva come: benchmark.sh

#!/bin/bash
MODELS=("llama3.1:8b" "mistral:7b")
PROMPT="Write a detailed 200-word product description for a wireless mechanical keyboard designed for programmers."

for model in "${MODELS[@]}"; do
    echo "=== Benchmarking: $model ==="

    # Riscaldamento (la prima esecuzione carica il modello in memoria)
    ollama run "$model" "Hello" > /dev/null 2>&1

    # Esecuzione cronometrata
    START=$(date +%s%N)
    RESULT=$(curl -s http://localhost:11434/api/generate -d "{
        \"model\": \"$model\",
        \"prompt\": \"$PROMPT\",
        \"stream\": false
    }")
    END=$(date +%s%N)

    DURATION=$(( (END - START) / 1000000 ))
    TOKENS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('eval_count', 'N/A'))")

    echo "Tempo: ${DURATION}ms"
    echo "Token generati: $TOKENS"
    if [ "$TOKENS" != "N/A" ] && [ "$DURATION" -gt 0 ]; then
        TPS=$(python3 -c "print(f'{$TOKENS / ($DURATION / 1000):.1f}')")
        echo "Velocità: $TPS token/secondo"
    fi
    echo ""
done
```

```bash
chmod +x benchmark.sh
./benchmark.sh
```

Annota i tuoi token/secondo per ogni modello. Questo numero determina quali workflow produttivi sono pratici per il tuo rig.

{@ insight stack_fit @}

**Requisiti di velocità per caso d'uso:**
- Elaborazione batch (asincrona): 5+ token/sec va bene (non ti interessa la latenza)
- Strumenti interattivi (l'utente aspetta): 20+ token/sec minimo
- API in tempo reale (rivolto ai clienti): 30+ token/sec per una buona UX
- Chat in streaming: 15+ token/sec per risultare reattivo

### Messa in Sicurezza del Tuo Server di Inferenza Locale

{? if computed.os_family == "windows" ?}
> **Nota Windows:** Ollama su Windows si collega a localhost di default. Verifica con `netstat -an | findstr 11434` in PowerShell. Usa Windows Firewall per bloccare l'accesso esterno alla porta 11434.
{? elif computed.os_family == "macos" ?}
> **Nota macOS:** Ollama su macOS si collega a localhost di default. Verifica con `lsof -i :11434`. Il firewall di macOS dovrebbe bloccare automaticamente le connessioni esterne.
{? endif ?}

La tua istanza Ollama non dovrebbe mai essere accessibile da internet a meno che tu non lo intenda esplicitamente.

```bash
# Verifica che Ollama sia in ascolto solo su localhost
ss -tlnp | grep 11434
# Dovrebbe mostrare 127.0.0.1:11434, NON 0.0.0.0:11434

# Se hai bisogno di accesso remoto (es., da un'altra macchina sulla tua LAN):
# Usa il tunneling SSH invece di esporre la porta
ssh -L 11434:localhost:11434 your-rig-ip

# Regole firewall (Linux)
sudo ufw deny in 11434
sudo ufw allow from 192.168.1.0/24 to any port 11434  # Solo LAN, se necessario
```

> **Errore Comune:** Collegare Ollama a 0.0.0.0 per "comodità" e dimenticarsene. Chiunque trovi il tuo IP può usare la tua GPU per inferenza gratuita. Peggio, possono estrarre pesi del modello e prompt di sistema. Sempre localhost. Sempre tunnel.

### Checkpoint Lezione 2

Dovresti ora avere:
- [ ] Ollama installato e in esecuzione
- [ ] Almeno un modello cavallo di battaglia scaricato (llama3.1:8b o equivalente)
- [ ] Un Modelfile personalizzato per il tuo carico di lavoro previsto
- [ ] Numeri di benchmark: token/secondo per ogni modello sul tuo rig
- [ ] Ollama collegato solo a localhost

*Nel corso STREETS completo, il Modulo T (Fossati Tecnici) ti mostra come costruire configurazioni di modelli proprietarie, pipeline di fine-tuning e toolchain personalizzate che i concorrenti non possono facilmente replicare. Il Modulo R (Motori di Guadagno) ti dà i servizi esatti da costruire sopra questo stack.*

---

## Lezione 3: Il Vantaggio della Privacy

*"La tua configurazione privata È un vantaggio competitivo — non solo una preferenza."*

### La Privacy È una Caratteristica del Prodotto, Non una Limitazione

La maggior parte degli sviluppatori configura l'infrastruttura locale perché valuta personalmente la privacy, o perché si diverte a sperimentare. Va bene. Ma stai lasciando soldi sul tavolo se non ti rendi conto che **la privacy è una delle caratteristiche più commerciabili nella tecnologia in questo momento.**

Ecco perché: ogni volta che un'azienda invia dati all'API di OpenAI, quei dati passano attraverso un terzo. Per molte imprese — specialmente quelle nella sanità, finanza, legale, governo e aziende con sede nell'UE — questo è un problema reale. Non teorico. Un problema del tipo "non possiamo usare questo strumento perché la compliance ha detto no".

Tu, che esegui modelli localmente sulla tua macchina, non hai quel problema.

### Il Vento in Poppa Normativo

L'ambiente normativo si muove nella tua direzione. Velocemente.

{? if regional.country == "US" ?}
> **Sede negli USA:** Le normative sotto che ti riguardano di più sono HIPAA, SOC 2, ITAR e le leggi sulla privacy a livello statale (California CCPA, ecc.). Le normative UE contano comunque — influenzano la tua capacità di servire clienti europei, che è un mercato remunerativo.
{? elif regional.country == "GB" ?}
> **Sede nel Regno Unito:** Post-Brexit, il Regno Unito ha il proprio framework di protezione dei dati (UK GDPR + Data Protection Act 2018). Il tuo vantaggio di elaborazione locale è particolarmente forte per servire i servizi finanziari UK e il lavoro adiacente al NHS.
{? elif regional.country == "DE" ?}
> **Sede in Germania:** Sei in uno degli ambienti di protezione dei dati più rigidi al mondo. Questo è un *vantaggio* — i clienti tedeschi già capiscono perché l'elaborazione locale conta, e pagheranno per questo.
{? elif regional.country == "AU" ?}
> **Sede in Australia:** Il Privacy Act 1988 e gli Australian Privacy Principles (APPs) governano i tuoi obblighi. L'elaborazione locale è un forte punto di vendita per clienti governativi e sanitari sotto il My Health Records Act.
{? endif ?}

**EU AI Act (applicato dal 2024-2026):**
- I sistemi IA ad alto rischio necessitano di pipeline di elaborazione dati documentate
- Le aziende devono dimostrare dove fluiscono i dati e chi li elabora
- L'elaborazione locale semplifica drasticamente la conformità
- Le aziende UE cercano attivamente fornitori di servizi IA che possano garantire la residenza dei dati nell'UE

**GDPR (già applicato):**
- "L'elaborazione dei dati" include l'invio di testo a un'API LLM
- Le aziende hanno bisogno di Accordi per il Trattamento dei Dati con ogni terza parte
- L'elaborazione locale elimina completamente la terza parte
- Questo è un vero punto di vendita: "I tuoi dati non lasciano mai la tua infrastruttura. Non c'è nessun DPA di terze parti da negoziare."

**Normative specifiche di settore:**
- **HIPAA (Sanità USA):** I dati dei pazienti non possono essere inviati ad API IA consumer senza un BAA (Business Associate Agreement). La maggior parte dei provider IA non offre BAA per l'accesso API. L'elaborazione locale aggira completamente questo problema.
- **SOC 2 (Enterprise):** Le aziende sottoposte a audit SOC 2 devono documentare ogni elaboratore di dati. Meno elaboratori = audit più facili.
- **ITAR (Difesa USA):** I dati tecnici controllati non possono lasciare la giurisdizione USA. I provider IA cloud con infrastruttura internazionale sono problematici.
- **PCI DSS (Finanza):** L'elaborazione dei dati delle carte di credito ha requisiti rigorosi su dove viaggiano i dati.

### Come Posizionare la Privacy nelle Conversazioni di Vendita

Non devi essere un esperto di conformità. Devi capire tre frasi e sapere quando usarle:

**Frase 1: "I tuoi dati non lasciano mai la tua infrastruttura."**
Usa quando: Parli con qualsiasi prospect attento alla privacy. Questo è il gancio universale.

**Frase 2: "Nessun accordo di trattamento dati con terze parti necessario."**
Usa quando: Parli con aziende europee o qualsiasi azienda con un team legale/compliance. Questo risparmia loro settimane di revisione legale.

**Frase 3: "Audit trail completo, elaborazione single-tenant."**
Usa quando: Parli con enterprise o industrie regolamentate. Devono dimostrare la loro pipeline IA agli auditor.

**Esempio di posizionamento (per la tua pagina servizi o le tue proposte):**

> "A differenza dei servizi IA basati sul cloud, [Il Tuo Servizio] elabora tutti i dati localmente su hardware dedicato. I tuoi documenti, codice e dati non lasciano mai l'ambiente di elaborazione. Non ci sono API di terze parti nella pipeline, nessun accordo di condivisione dati da negoziare, e log di audit completi di ogni operazione. Questo rende [Il Tuo Servizio] adatto per organizzazioni con requisiti rigorosi di gestione dati, inclusi ambienti di conformità GDPR, HIPAA e SOC 2."

Quel paragrafo, su una landing page, attirerà esattamente i clienti che pagheranno tariffe premium.

### La Giustificazione del Prezzo Premium

Ecco il business case in numeri concreti:

**Servizio di elaborazione IA standard (usando API cloud):**
- I dati del cliente vanno a OpenAI/Anthropic/Google
- Stai competendo con ogni sviluppatore che può chiamare un'API
- Tariffa di mercato: $0.01-0.05 per documento elaborato
- Stai essenzialmente rivendendo l'accesso API con un markup

**Servizio di elaborazione IA privacy-first (il tuo stack locale):**
- I dati del cliente restano sulla tua macchina
- Stai competendo con un pool molto più piccolo di fornitori
- Tariffa di mercato: $0.10-0.50 per documento elaborato (premium 5-10x)
- Stai vendendo infrastruttura + competenza + conformità

Il premium della privacy è reale: **da 5x a 10x** rispetto ai servizi cloud commodity per lo stesso task sottostante. E i clienti che lo pagano sono più fedeli, meno sensibili al prezzo e hanno budget più grandi.

{@ insight competitive_position @}

### Configurare Workspace Isolati

Se hai un lavoro diurno (la maggior parte di voi ce l'ha), hai bisogno di una separazione netta tra il lavoro per il datore di lavoro e il lavoro produttivo. Questa non è solo protezione legale — è igiene operativa.

{? if computed.os_family == "windows" ?}
> **Consiglio Windows:** Crea un account utente Windows separato per il lavoro produttivo (Impostazioni > Account > Famiglia e altri utenti > Aggiungi un altro utente). Questo ti dà un ambiente completamente isolato — profili browser separati, percorsi file separati, variabili d'ambiente separate. Passa tra account con Win+L.
{? endif ?}

**Opzione 1: Account utente separati (consigliato)**

```bash
# Linux: Crea un utente dedicato per il lavoro produttivo
sudo useradd -m -s /bin/bash income
sudo passwd income

# Passa all'utente income per tutto il lavoro di guadagno
su - income

# Tutti i progetti di guadagno, chiavi API e dati vivono sotto /home/income/
```

**Opzione 2: Workspace containerizzati**

```bash
# Isolamento basato su Docker
# Crea un container di workspace dedicato

# docker-compose.yml
version: '3.8'
services:
  income-workspace:
    image: ubuntu:22.04
    volumes:
      - ./income-projects:/workspace
      - ./income-data:/data
    environment:
      - OLLAMA_HOST=host.docker.internal:11434
    network_mode: bridge
    # La VPN, gli strumenti, ecc. del tuo datore di lavoro NON sono in questo container
```

**Opzione 3: Macchina fisica separata (la più blindata)**

Se fai sul serio e il tuo reddito lo giustifica, una macchina dedicata elimina tutte le domande. Un Dell OptiPlex usato con una RTX 3060 costa $400-600 e si ripaga nel primo mese di lavoro per clienti.

**Checklist di separazione minima:**
- [ ] Progetti di guadagno in una directory separata (mai mescolati con repo del datore di lavoro)
- [ ] Chiavi API separate per il lavoro di guadagno (mai usare chiavi fornite dal datore di lavoro)
- [ ] Profilo browser separato per account legati al guadagno
- [ ] Il lavoro di guadagno non viene mai fatto su hardware del datore di lavoro
- [ ] Il lavoro di guadagno non viene mai fatto sulla rete del datore di lavoro (usa il tuo internet personale o una VPN)
- [ ] Account GitHub/GitLab separato per i progetti di guadagno (opzionale ma pulito)

> **Errore Comune:** Usare la chiave API OpenAI del tuo datore di lavoro "solo per testare" il tuo progetto personale. Questo crea una traccia cartacea che la dashboard di fatturazione del tuo datore di lavoro può vedere, e confonde le acque sulla proprietà intellettuale. Prendi le tue chiavi. Sono economiche.

### Checkpoint Lezione 3

Dovresti ora comprendere:
- [ ] Perché la privacy è una caratteristica di prodotto commerciabile, non solo una preferenza personale
- [ ] Quali normative creano domanda per l'elaborazione IA locale
- [ ] Tre frasi da usare nelle conversazioni di vendita sulla privacy
- [ ] Come i servizi privacy-first comandano un prezzo premium 5-10x
- [ ] Come separare il lavoro di guadagno dal lavoro per il datore di lavoro

*Nel corso STREETS completo, il Modulo E (Vantaggio in Evoluzione) ti insegna come tracciare i cambiamenti normativi e posizionarti davanti ai nuovi requisiti di conformità prima che i tuoi concorrenti sappiano della loro esistenza.*

---

## Lezione 4: Il Minimo Legale

*"Quindici minuti di preparazione legale adesso prevengono mesi di problemi dopo."*

### Questo Non È un Consiglio Legale

Sono uno sviluppatore, non un avvocato. Ciò che segue è una checklist pratica che la maggior parte degli sviluppatori nella maggior parte delle situazioni dovrebbe affrontare. Se la tua situazione è complessa (equity nel tuo datore di lavoro, non-compete con termini specifici, ecc.), spendi $200 per una consulenza di 30 minuti con un avvocato del lavoro. È il miglior ROI che otterrai.

### Passo 1: Leggi il Tuo Contratto di Lavoro

Trova il tuo contratto di lavoro o lettera di assunzione. Cerca queste sezioni:

**Clausola di cessione della proprietà intellettuale** — Cerca linguaggio come:
- "Tutte le invenzioni, sviluppi e prodotti del lavoro..."
- "...creati durante il periodo di impiego..."
- "...relativi all'attività della Società o attività prevista..."

**Frasi chiave che ti limitano:**
- "Tutto il prodotto del lavoro creato durante l'impiego appartiene alla Società" (ampio — potenzialmente problematico)
- "Prodotto del lavoro creato usando risorse della Società" (più ristretto — di solito va bene se usi la tua attrezzatura)
- "Relativo all'attività corrente o prevista della Società" (dipende da cosa fa il tuo datore di lavoro)

**Frasi chiave che ti liberano:**
- "Escludendo il lavoro fatto interamente nel tempo libero del Dipendente con le risorse proprie del Dipendente e non correlato all'attività della Società" (questa è la tua deroga — molti stati USA la richiedono)
- Alcuni stati (California, Washington, Minnesota, Illinois, altri) hanno leggi che limitano le pretese di PI del datore di lavoro sui progetti personali, indipendentemente da cosa dice il contratto.

### Il Test delle 3 Domande

Per qualsiasi progetto di guadagno, chiediti:

1. **Tempo:** Stai facendo questo lavoro nel tuo tempo libero? (Non durante l'orario di lavoro, non durante turni di reperibilità)
2. **Attrezzatura:** Stai usando il tuo hardware, il tuo internet, le tue chiavi API? (Non il laptop del datore di lavoro, non la VPN del datore di lavoro, non gli account cloud del datore di lavoro)
3. **Materia:** Questo è non correlato all'attività del tuo datore di lavoro? (Se lavori in un'azienda di IA sanitaria e vuoi vendere servizi di IA sanitaria... è un problema. Se lavori in un'azienda di IA sanitaria e vuoi vendere elaborazione documenti per agenti immobiliari... va bene.)

Se tutte e tre le risposte sono pulite, sei quasi certamente a posto. Se qualsiasi risposta è ambigua, chiarisci prima di procedere.

> **Parliamoci Chiaro:** La stragrande maggioranza degli sviluppatori che fanno lavoro extra non ha mai problemi. I datori di lavoro si preoccupano di proteggere i vantaggi competitivi, non di impedirti di guadagnare soldi extra su progetti non correlati. Ma "quasi certamente a posto" non è "sicuramente a posto." Se il tuo contratto è insolitamente ampio, fai una conversazione con il tuo manager o le HR — o consulta un avvocato. Il lato negativo del non controllare è molto peggio dell'imbarazzo moderato di chiedere.

### Passo 2: Scegli una Struttura Aziendale

Hai bisogno di un'entità legale per separare i tuoi beni personali dalle tue attività commerciali, e per aprire la porta a conti bancari aziendali, elaborazione pagamenti e benefici fiscali.

{? if regional.country ?}
> **La tua posizione: {= regional.country | fallback("Sconosciuto") =}.** Il tipo di entità raccomandato per la tua regione è una **{= regional.business_entity_type | fallback("LLC o equivalente") =}**, con costi di registrazione tipici di {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Scorri alla sezione del tuo paese qui sotto, o leggi tutte le sezioni per capire come operano i clienti in altre regioni.
{? endif ?}

{? if regional.country == "US" ?}
#### Stati Uniti (La Tua Regione)
{? else ?}
#### Stati Uniti
{? endif ?}

| Struttura | Costo | Protezione | Migliore Per |
|-----------|-------|------------|--------------|
| **Ditta Individuale** (default) | $0 | Nessuna (responsabilità personale) | Testare il terreno. Primi $1K. |
| **LLC a Membro Singolo** | $50-500 (varia per stato) | Protezione dei beni personali | Lavoro di reddito attivo. La maggior parte degli sviluppatori dovrebbe iniziare qui. |
| **Elezione S-Corp** (su una LLC) | Costo LLC + $0 per l'elezione | Stessa della LLC + benefici fiscali sui contributi | Quando guadagni costantemente $40K+/anno da questo |

**Consigliato per sviluppatori USA:** LLC a Membro Singolo nel tuo stato di residenza.

**Stati più economici per la costituzione:** Wyoming ($100, nessuna imposta sul reddito statale), New Mexico ($50), Montana ($70). Ma costituire nel tuo stato di residenza è di solito il più semplice a meno che tu non abbia un motivo specifico per non farlo.

**Come fare:**
1. Vai al sito web del Secretary of State del tuo stato
2. Cerca "form LLC" o "business entity filing"
3. Compila gli Articles of Organization (modulo da 10 minuti)
4. Ottieni un EIN dall'IRS (gratuito, richiede 5 minuti su irs.gov)

{? if regional.country == "GB" ?}
#### Regno Unito (La Tua Regione)
{? else ?}
#### Regno Unito
{? endif ?}

| Struttura | Costo | Protezione | Migliore Per |
|-----------|-------|------------|--------------|
| **Sole Trader** | Gratuito (registrazione con HMRC) | Nessuna | Primo reddito. Test. |
| **Limited Company (Ltd)** | ~$15 via Companies House | Protezione dei beni personali | Qualsiasi lavoro di reddito serio. |

**Consigliato:** Ltd company via Companies House. Richiede circa 20 minuti e costa GBP 12.

#### Unione Europea

Varia significativamente per paese, ma lo schema generale:

- **Germania:** Einzelunternehmer (ditta individuale) per iniziare, GmbH per lavoro serio (ma la GmbH richiede EUR 25.000 di capitale — considera UG per EUR 1)
- **Paesi Bassi:** Eenmanszaak (ditta individuale, registrazione gratuita) o BV (comparabile alla Ltd)
- **Francia:** Micro-entrepreneur (semplificato, consigliato per iniziare)
- **Estonia:** e-Residency + OUE (popolare per non residenti, completamente online)

{? if regional.country == "AU" ?}
#### Australia (La Tua Regione)
{? else ?}
#### Australia
{? endif ?}

| Struttura | Costo | Protezione | Migliore Per |
|-----------|-------|------------|--------------|
| **Sole Trader** | ABN gratuito | Nessuna | Per iniziare |
| **Pty Ltd** | ~AUD 500-800 via ASIC | Protezione dei beni personali | Reddito serio |

**Consigliato:** Inizia con un ABN da Sole Trader (gratuito, istantaneo), passa a Pty Ltd quando guadagni costantemente.

### Passo 3: Elaborazione Pagamenti (setup di 15 minuti)

Hai bisogno di un modo per essere pagato. Configuralo adesso, non quando il tuo primo cliente sta aspettando.

{? if regional.payment_processors ?}
> **Consigliato per {= regional.country | fallback("la tua regione") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (consigliato per la maggior parte degli sviluppatori):**

```
1. Vai a stripe.com
2. Crea un account con la tua email aziendale
3. Completa la verifica dell'identità
4. Collega il tuo conto bancario aziendale
5. Ora puoi accettare pagamenti, creare fatture e impostare abbonamenti
```

Tempo: ~15 minuti. Puoi iniziare ad accettare pagamenti immediatamente (Stripe trattiene i fondi per 7 giorni sui nuovi account).

**Lemon Squeezy (consigliato per prodotti digitali):**

Se stai vendendo prodotti digitali (template, strumenti, corsi, SaaS), Lemon Squeezy agisce come Merchant of Record. Questo significa:
- Gestiscono la tassa sulle vendite, IVA e GST per te globalmente
- Non devi registrarti per l'IVA nell'UE
- Gestiscono rimborsi e contestazioni

```
1. Vai a lemonsqueezy.com
2. Crea un account
3. Configura il tuo negozio
4. Aggiungi i prodotti
5. Loro gestiscono tutto il resto
```

**Stripe Atlas (per sviluppatori internazionali o chi vuole un'entità USA):**

Se sei fuori dagli USA ma vuoi vendere a clienti USA con un'entità USA:
- $500 una tantum
- Crea una LLC Delaware per te
- Configura un conto bancario USA (via Mercury o Stripe)
- Fornisce servizio di agente registrato
- Richiede circa 1-2 settimane

### Passo 4: Privacy Policy e Termini di Servizio

Se stai vendendo qualsiasi servizio o prodotto online, ti servono questi. Non pagare un avvocato per il boilerplate.

**Fonti gratuite e affidabili per template:**
- **Termly.io** — Generatore gratuito di privacy policy e ToS. Rispondi alle domande, ottieni i documenti.
- **Avodocs.com** — Documenti legali open-source per startup. Gratuiti.
- **choosealicense.com di GitHub** — Per licenze di progetti open-source specificamente.
- **Policy open-source di Basecamp** — Cerca "Basecamp open source policies" — buoni template in linguaggio semplice.

**Cosa deve coprire la tua privacy policy (se elabori dati dei clienti):**
- Quali dati raccogli
- Come li elabori (localmente — questo è il tuo vantaggio)
- Per quanto tempo li conservi
- Come i clienti possono richiedere la cancellazione
- Se terze parti accedono ai dati (idealmente: nessuna)

**Tempo:** 30 minuti con un generatore di template. Fatto.

### Passo 5: Conto Bancario Separato

Non far passare il reddito aziendale attraverso il tuo conto corrente personale. Le ragioni:

1. **Chiarezza fiscale:** Quando arriva il momento delle tasse, devi sapere esattamente cosa era reddito aziendale e cosa no.
2. **Protezione legale:** Se hai una LLC, mescolare fondi personali e aziendali può "perforare il velo societario" — significa che un tribunale può ignorare la protezione di responsabilità della tua LLC.
3. **Professionalità:** Fatture da "Consulenza di Mario Srl" che arrivano su un conto aziendale dedicato sembrano legittime. Pagamenti sul tuo PayPal personale no.

**Conto bancario aziendale gratuito o a basso costo:**
{? if regional.country == "US" ?}
- **Mercury** (consigliato per te) — Gratuito, progettato per startup. Eccellente API se vuoi automatizzare la contabilità in seguito.
- **Relay** — Gratuito, buono per separare i flussi di reddito in sotto-conti.
{? elif regional.country == "GB" ?}
- **Starling Bank** (consigliato per te) — Conto aziendale gratuito, setup istantaneo.
- **Wise Business** — Multi-valuta a basso costo. Ottimo se servi clienti internazionali.
{? else ?}
- **Mercury** (USA) — Gratuito, progettato per startup. Eccellente API se vuoi automatizzare la contabilità in seguito.
- **Relay** (USA) — Gratuito, buono per separare i flussi di reddito in sotto-conti.
- **Starling Bank** (UK) — Conto aziendale gratuito.
{? endif ?}
- **Wise Business** (Internazionale) — Multi-valuta a basso costo. Ottimo per ricevere pagamenti in USD, EUR, GBP, ecc.
- **Qonto** (UE) — Conto bancario aziendale pulito per aziende europee.

Apri il conto adesso. Richiede 10-15 minuti online e 1-3 giorni per la verifica.

### Passo 6: Basi Fiscali per il Reddito Extra degli Sviluppatori

{? if regional.tax_note ?}
> **Nota fiscale per {= regional.country | fallback("la tua regione") =}:** {= regional.tax_note | fallback("Consulta un professionista fiscale locale per i dettagli.") =}
{? endif ?}

> **Parliamoci Chiaro:** Le tasse sono la cosa che la maggior parte degli sviluppatori ignora fino ad aprile, e poi va nel panico. Spendere 30 minuti adesso ti fa risparmiare soldi veri e stress.

**Stati Uniti:**
- Il reddito extra oltre $400/anno richiede la tassa di lavoro autonomo (~15.3% per Social Security + Medicare)
- Più la tua aliquota regolare sull'imposta sul reddito sull'utile netto
- **Tasse stimate trimestrali:** Se dovrai più di $1.000 in tasse, l'IRS si aspetta pagamenti trimestrali (15 aprile, 15 giugno, 15 settembre, 15 gennaio). Il pagamento insufficiente comporta sanzioni.
- Metti da parte **il 25-30%** del reddito netto per le tasse. Mettilo in un conto risparmio separato immediatamente.

**Detrazioni comuni per il reddito extra degli sviluppatori:**
- Costi API (OpenAI, Anthropic, ecc.) — 100% deducibili
- Acquisti hardware usati per l'attività — ammortizzabili o deduzione Sezione 179
- Costo dell'elettricità attribuibile all'uso aziendale
- Abbonamenti software usati per il lavoro produttivo
- Deduzione per ufficio domestico (semplificata: $5/sq ft, fino a 300 sq ft = $1.500)
- Internet (percentuale di uso aziendale)
- Nomi di dominio, hosting, servizi email
- Sviluppo professionale (corsi, libri) relativi al tuo lavoro produttivo

**Regno Unito:**
- Dichiarazione tramite Self Assessment
- Reddito commerciale sotto GBP 1.000: esentasse (Trading Allowance)
- Oltre: paghi Income Tax + Class 4 NICs sui profitti
- Date di pagamento: 31 gennaio e 31 luglio

**Traccia tutto dal primo giorno.** Usa un semplice foglio di calcolo se nient'altro:

```
| Data       | Categoria   | Descrizione            | Importo | Tipo    |
|------------|-------------|------------------------|---------|---------|
| 2025-01-15 | API         | Credito Anthropic      | -$20.00 | Spesa   |
| 2025-01-18 | Ricavo      | Fattura cliente #001   | +$500.00| Entrata |
| 2025-01-20 | Software    | Piano Vercel Pro       | -$20.00 | Spesa   |
| 2025-01-20 | Riserva Tax | 30% del reddito netto  | -$138.00| Trasf.  |
```

> **Errore Comune:** "Le tasse le capirò dopo." Dopo è il Q4, devi $3.000 in tasse stimate più sanzioni, e hai già speso i soldi. Automatizza: ogni volta che il reddito arriva sul tuo conto aziendale, trasferisci il 30% su un conto risparmio tasse immediatamente.

### Checkpoint Lezione 4

Dovresti ora avere (o avere un piano per):
- [ ] Letto la clausola PI del tuo contratto di lavoro
- [ ] Superato il Test delle 3 Domande per il tuo lavoro di guadagno pianificato
- [ ] Scelta una struttura aziendale (o deciso di iniziare come ditta individuale)
- [ ] Elaborazione pagamenti configurata (Stripe o Lemon Squeezy)
- [ ] Privacy policy e ToS da un generatore di template
- [ ] Conto bancario aziendale separato (o richiesta inviata)
- [ ] Strategia fiscale: accantonamento del 30% + calendario pagamenti trimestrali

*Nel corso STREETS completo, il Modulo E (Playbook di Esecuzione) include template di modellazione finanziaria che calcolano automaticamente i tuoi obblighi fiscali, la redditività del progetto e i punti di pareggio per ogni motore di guadagno.*

---

## Lezione 5: Il Budget di {= regional.currency_symbol | fallback("$") =}200/mese

*"La tua attività ha un burn rate. Conoscilo. Controllalo. Fallo rendere."*

### Perché {= regional.currency_symbol | fallback("$") =}200/mese

Duecento {= regional.currency | fallback("dollari") =} al mese è il budget minimo viabile per un'operazione di reddito per sviluppatori. È abbastanza per gestire servizi reali, servire clienti reali e generare ricavi reali. È anche abbastanza piccolo che se niente funziona, non hai puntato tutto.

L'obiettivo è semplice: **trasformare {= regional.currency_symbol | fallback("$") =}200/mese in {= regional.currency_symbol | fallback("$") =}600+/mese entro 90 giorni.** Se riesci a farlo, hai un'attività. Se non riesci, cambi strategia — non aumenti il budget.

### La Ripartizione del Budget

#### Livello 1: Crediti API — $50-100/mese

Questo è il tuo calcolo di produzione per la qualità rivolta ai clienti.

**Allocazione iniziale consigliata:**

```
Anthropic (Claude):     $40/mese  — Il tuo principale per output di qualità
OpenAI (GPT-4o-mini):   $20/mese  — Lavoro di volume economico, fallback
DeepSeek:               $10/mese  — Task economici, sperimentazione
Buffer:                 $30/mese  — Overflow o test di nuovi provider
```

**Come gestire la spesa API:**

```python
# Semplice tracker del budget API — esegui giornalmente via cron
# Salva come: check_api_spend.py

import requests
import json
from datetime import datetime

# Controlla l'utilizzo Anthropic
# (Anthropic fornisce l'utilizzo nella dashboard; ecco come tracciare localmente)

MONTHLY_BUDGET = {
    "anthropic": 40.00,
    "openai": 20.00,
    "deepseek": 10.00,
}

# Traccia localmente registrando ogni costo di chiamata API
USAGE_LOG = "api_usage.jsonl"

def get_monthly_spend(provider: str) -> float:
    """Calcola la spesa del mese corrente per un provider."""
    current_month = datetime.now().strftime("%Y-%m")
    total = 0.0
    try:
        with open(USAGE_LOG, "r") as f:
            for line in f:
                entry = json.loads(line)
                if entry["provider"] == provider and entry["date"].startswith(current_month):
                    total += entry["cost"]
    except FileNotFoundError:
        pass
    return total

def log_api_call(provider: str, tokens_in: int, tokens_out: int, model: str):
    """Registra una chiamata API per il tracking del budget."""
    # Costo per 1M token (aggiorna questi quando cambiano i prezzi)
    PRICING = {
        "claude-3.5-sonnet": {"input": 3.00, "output": 15.00},
        "claude-3.5-haiku": {"input": 0.80, "output": 4.00},
        "gpt-4o-mini": {"input": 0.15, "output": 0.60},
        "gpt-4o": {"input": 2.50, "output": 10.00},
        "deepseek-v3": {"input": 0.27, "output": 1.10},
    }

    prices = PRICING.get(model, {"input": 1.0, "output": 5.0})
    cost = (tokens_in / 1_000_000 * prices["input"]) + \
           (tokens_out / 1_000_000 * prices["output"])

    entry = {
        "date": datetime.now().isoformat(),
        "provider": provider,
        "model": model,
        "tokens_in": tokens_in,
        "tokens_out": tokens_out,
        "cost": round(cost, 6),
    }

    with open(USAGE_LOG, "a") as f:
        f.write(json.dumps(entry) + "\n")

    # Avviso budget
    monthly_spend = get_monthly_spend(provider)
    budget = MONTHLY_BUDGET.get(provider, 0)
    if monthly_spend > budget * 0.8:
        print(f"ATTENZIONE: spesa {provider} a {monthly_spend:.2f}/{budget:.2f} "
              f"({monthly_spend/budget*100:.0f}%)")

    return cost
```

**La strategia di spesa ibrida:**
- Usa LLM locali per l'80% dell'elaborazione (classificazione, estrazione, riassunti, bozze)
- Usa chiamate API per il 20% dell'elaborazione (passaggio finale di qualità, ragionamento complesso, output rivolto ai clienti)
- Il tuo costo effettivo per task cala drasticamente rispetto all'uso puro di API

{? if computed.monthly_electricity_estimate ?}
> **Il tuo costo elettricità stimato:** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/mese per operatività 24/7 a {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh. Questo è già considerato nel tuo costo operativo effettivo.
{? endif ?}

#### Livello 2: Infrastruttura — {= regional.currency_symbol | fallback("$") =}30-50/mese

```
Nome di dominio:        $12/anno ($1/mese)     — Namecheap, Cloudflare, Porkbun
Email (aziendale):      $0-6/mese              — Zoho Mail gratuito, o Google Workspace $6
VPS (opzionale):        $5-20/mese             — Per ospitare servizi leggeri
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/mese                — Cloudflare piano gratuito
Hosting (statico):      $0/mese                — Vercel, Netlify, Cloudflare Pages (piani gratuiti)
```

**Ti serve un VPS?**

Se il tuo modello di reddito è:
- **Vendere prodotti digitali:** No. Ospita su Vercel/Netlify gratuitamente. Usa Lemon Squeezy per la consegna.
- **Elaborazione asincrona per clienti:** Forse. Puoi eseguire job sul tuo rig locale e consegnare risultati. Un VPS aggiunge affidabilità.
- **Offrire un servizio API:** Sì, probabilmente. Un VPS da $5-10 funge da gateway API leggero, anche se l'elaborazione pesante avviene sulla tua macchina locale.
- **Vendere SaaS:** Sì. Ma inizia con il piano più economico e scala.

**Infrastruttura iniziale consigliata:**

```
Rig locale — calcolo principale, inferenza LLM, elaborazione pesante
   |
   +-- Tunnel SSH o VPN WireGuard
   |
VPS $5 (Hetzner/DigitalOcean) — gateway API, ricevitore webhook, hosting statico
   |
   +-- Cloudflare (gratuito) — DNS, CDN, protezione DDoS
   |
Vercel/Netlify (gratuito) — sito marketing, landing page, documentazione
```

Costo totale infrastruttura: $5-20/mese. Il resto sono piani gratuiti.

#### Livello 3: Strumenti — {= regional.currency_symbol | fallback("$") =}20-30/mese

```
Analytics:              $0/mese    — Plausible Cloud ($9) o self-hosted,
                                      o Vercel Analytics (piano gratuito)
                                      o semplicemente Cloudflare analytics (gratuito)
Email marketing:        $0/mese    — Buttondown (gratuito fino a 100 iscritti),
                                      Resend ($0 per 3K email/mese)
Monitoraggio:           $0/mese    — UptimeRobot (gratuito, 50 monitor),
                                      Better Stack (piano gratuito)
Design:                 $0/mese    — Figma (gratuito), Canva (piano gratuito)
Contabilità:            $0/mese    — Wave (gratuito), o un foglio di calcolo
                                      Hledger (gratuito, contabilità in testo semplice)
```

> **Parliamoci Chiaro:** Puoi gestire l'intero stack di strumenti su piani gratuiti quando inizi. I $20-30 allocati qui sono per quando superi i piani gratuiti o vuoi una funzionalità premium specifica. Non spenderli solo perché sono nel budget. Il budget non speso è profitto.

#### Livello 4: Riserva — {= regional.currency_symbol | fallback("$") =}0-30/mese

Questo è il tuo fondo "cose che non ho previsto":
- Un picco di costi API da un job batch inaspettatamente grande
- Uno strumento di cui hai bisogno per un progetto specifico di un cliente
- Acquisto di dominio d'emergenza quando trovi il nome perfetto
- Un acquisto una tantum (tema, template, set di icone)

Se non usi la riserva, si accumula. Dopo 3 mesi di riserva inutilizzata, considera di riallocare a crediti API o infrastruttura.

### Il Calcolo del ROI

Questo è l'unico numero che conta:

```
Ricavi Mensili - Costi Mensili = Profitto Netto
Profitto Netto / Costi Mensili = Multiplo ROI

Esempio:
$600 ricavi - $200 costi = $400 profitto
$400 / $200 = 2x ROI

L'obiettivo: 3x ROI ($600+ ricavi su $200 di spesa)
Il minimo: 1x ROI ($200 ricavi = pareggio)
Sotto 1x: Cambia strategia o riduci i costi
```

{@ insight cost_projection @}

**Quando aumentare il budget:**

Aumenta il tuo budget SOLO quando:
1. Sei costantemente a 2x+ ROI per 2+ mesi
2. Più spesa aumenterebbe direttamente i ricavi (es., più crediti API = più capacità per i clienti)
3. L'aumento è legato a un flusso di reddito specifico e testato

**Quando NON aumentare il budget:**
- "Penso che questo nuovo strumento aiuterà" (testa prima le alternative gratuite)
- "Tutti dicono che devi spendere soldi per fare soldi" (non a questo stadio)
- "Un VPS più grande renderà il mio servizio più veloce" (la velocità è davvero il collo di bottiglia?)
- Non hai ancora raggiunto 1x ROI (sistema i ricavi, non la spesa)

**La scala di crescita:**

```
$200/mese  → Provare il concetto (mesi 1-3)
$500/mese  → Scalare ciò che funziona (mesi 4-6)
$1000/mese → Flussi di reddito multipli (mesi 6-12)
$2000+/mese → Operazione aziendale completa (anno 2+)

Ogni passo richiede di provare il ROI al livello attuale prima.
```

> **Errore Comune:** Trattare i {= regional.currency_symbol | fallback("$") =}200 come un "investimento" che non deve restituire soldi immediatamente. No. Questo è un esperimento con una scadenza di 90 giorni. Se {= regional.currency_symbol | fallback("$") =}200/mese non generano {= regional.currency_symbol | fallback("$") =}200/mese di ricavi entro 90 giorni, qualcosa nella strategia deve cambiare. I soldi, il mercato, l'offerta — qualcosa non funziona. Sii onesto con te stesso.

### Checkpoint Lezione 5

Dovresti ora avere:
- [ ] Un budget mensile di ~$200 allocato su quattro livelli
- [ ] Account API creati con limiti di spesa impostati
- [ ] Decisioni infrastrutturali prese (solo locale vs. locale + VPS)
- [ ] Uno stack di strumenti selezionato (principalmente piani gratuiti per iniziare)
- [ ] Obiettivi ROI: 3x entro 90 giorni
- [ ] Una regola chiara: aumenta il budget solo dopo aver provato il ROI

*Nel corso STREETS completo, il Modulo E (Playbook di Esecuzione) include un template di dashboard finanziaria che traccia la tua spesa, i ricavi e il ROI per motore di guadagno in tempo reale — così sai sempre quali flussi sono redditizi e quali necessitano aggiustamenti.*

---

## Lezione 6: Il Tuo Documento dello Stack Sovrano

*"Ogni attività ha un piano. Questo è il tuo — e sta in due pagine."*

### Il Deliverable

Questa è la cosa più importante che creerai nel Modulo S. Il tuo Documento dello Stack Sovrano è un riferimento unico che cattura tutto sulla tua infrastruttura per generare reddito. Lo riferirai per tutto il resto del corso STREETS, lo aggiornerai man mano che la tua configurazione evolve, e lo userai per prendere decisioni lucide su cosa costruire e cosa saltare.

Crea un nuovo file. Markdown, Google Doc, pagina Notion, testo semplice — qualunque cosa manterrai effettivamente. Usa il template sotto, compilando ogni campo con i numeri e le decisioni delle Lezioni 1-5.

### Il Template

{? if computed.profile_completeness != "0" ?}
> **Vantaggio iniziale:** 4DA ha già rilevato alcune delle tue specifiche hardware e info sullo stack. Cerca i suggerimenti pre-compilati sotto — ti faranno risparmiare tempo nel compilare il template.
{? endif ?}

Copia l'intero template e compilalo. Ogni campo. Niente scorciatoie.

```markdown
# Documento dello Stack Sovrano
# [Il Tuo Nome o Nome dell'Attività]
# Creato: [Data]
# Ultimo Aggiornamento: [Data]

---

## 1. INVENTARIO HARDWARE

### Macchina Principale
- **Tipo:** [Desktop / Laptop / Mac / Server]
- **CPU:** [Modello] — [X] core, [X] thread
- **RAM:** [X] GB [DDR4/DDR5]
- **GPU:** [Modello] — [X] GB VRAM (o "Nessuna — inferenza solo CPU")
- **Storage:** [X] GB SSD liberi / [X] GB totali
- **OS:** [Distribuzione Linux / versione macOS / versione Windows]

### Rete
- **Download:** [X] Mbps
- **Upload:** [X] Mbps
- **Latenza verso API cloud:** [X] ms
- **Affidabilità ISP:** [Stabile / Interruzioni occasionali / Inaffidabile]

### Capacità di Uptime
- **Può funzionare 24/7:** [Sì / No — motivo]
- **UPS:** [Sì / No]
- **Accesso remoto:** [SSH / RDP / Tailscale / Nessuno]

### Costo Infrastruttura Mensile
- **Elettricità (stima 24/7):** $[X]/mese
- **Internet:** $[X]/mese (porzione aziendale)
- **Costo infrastruttura fisso totale:** $[X]/mese

---

## 2. STACK LLM

### Modelli Locali (via Ollama)
| Modello | Dimensione | Token/sec | Caso d'Uso |
|---------|-----------|-----------|------------|
| [es., llama3.1:8b] | [X]B | [X] tok/s | [es., Classificazione, estrazione] |
| [es., mistral:7b] | [X]B | [X] tok/s | [es., Riassunti, bozze] |
| [es., deepseek-coder] | [X]B | [X] tok/s | [es., Generazione codice] |

### Modelli API (per output di qualità critica)
| Provider | Modello | Budget Mensile | Caso d'Uso |
|----------|---------|---------------|------------|
| [es., Anthropic] | [Claude 3.5 Sonnet] | $[X] | [es., Contenuti rivolti ai clienti] |
| [es., OpenAI] | [GPT-4o-mini] | $[X] | [es., Fallback elaborazione volume] |

### Strategia di Inferenza
- **Il locale gestisce:** [X]% delle richieste ([elenco task])
- **Le API gestiscono:** [X]% delle richieste ([elenco task])
- **Costo misto stimato per 1M token:** $[X]

---

## 3. BUDGET MENSILE

| Categoria | Allocazione | Effettivo (aggiorna mensilmente) |
|-----------|-----------|--------------------------------|
| Crediti API | $[X] | $[  ] |
| Infrastruttura (VPS, dominio, email) | $[X] | $[  ] |
| Strumenti (analytics, email marketing) | $[X] | $[  ] |
| Riserva | $[X] | $[  ] |
| **Totale** | **$[X]** | **$[  ]** |

### Obiettivo di Ricavo
- **Mese 1-3:** $[X]/mese (minimo: coprire i costi)
- **Mese 4-6:** $[X]/mese
- **Mese 7-12:** $[X]/mese

---

## 4. STATO LEGALE

- **Stato occupazionale:** [Dipendente / Freelance / Tra un lavoro e l'altro]
- **Clausola PI revisionata:** [Sì / No / N/A]
- **Livello di rischio clausola PI:** [Pulita / Ambigua — da approfondire / Restrittiva]
- **Entità aziendale:** [LLC / Ltd / Ditta Individuale / Ancora nessuna]
  - **Stato/Paese:** [Dove registrata]
  - **Codice Fiscale/P.IVA:** [Ottenuto / In attesa / Non necessario ancora]
- **Elaborazione pagamenti:** [Stripe / Lemon Squeezy / Altro] — [Attivo / In attesa]
- **Conto bancario aziendale:** [Aperto / In attesa / Uso personale (sistema questo)]
- **Privacy policy:** [Fatta / Non ancora — URL: ___]
- **Termini di servizio:** [Fatti / Non ancora — URL: ___]

---

## 5. INVENTARIO TEMPO

- **Ore disponibili a settimana per progetti di reddito:** [X] ore
  - **Mattine giorni feriali:** [X] ore
  - **Sere giorni feriali:** [X] ore
  - **Fine settimana:** [X] ore
- **Fuso orario:** [Il tuo fuso orario]
- **Migliori blocchi di lavoro profondo:** [es., "Sabato 6-12, sere feriali 20-22"]

### Piano di Allocazione Tempo
| Attività | Ore/settimana |
|----------|--------------|
| Costruzione/coding | [X] |
| Marketing/vendite | [X] |
| Lavoro per clienti/consegna | [X] |
| Apprendimento/sperimentazione | [X] |
| Admin (fatturazione, email, ecc.) | [X] |

> Regola: Non allocare mai più del 70% del tempo disponibile.
> La vita succede. Il burnout è reale. Lascia un buffer.

---

## 6. INVENTARIO COMPETENZE

### Competenze Principali (cose che potresti insegnare ad altri)
1. [Competenza] — [anni di esperienza]
2. [Competenza] — [anni di esperienza]
3. [Competenza] — [anni di esperienza]

### Competenze Secondarie (competente ma non esperto)
1. [Competenza]
2. [Competenza]
3. [Competenza]

### In Esplorazione (stai imparando ora o vuoi imparare)
1. [Competenza]
2. [Competenza]

### Combinazioni Uniche
Cosa rende la TUA combinazione di competenze insolita? (Questo diventa il tuo fossato nel Modulo T)
- [es., "Conosco sia Rust CHE gli standard dei dati sanitari — pochissime persone hanno entrambi"]
- [es., "Posso costruire app full-stack E capisco la logistica della supply chain da una carriera precedente"]
- [es., "Parlo correntemente 3 lingue E so programmare — posso servire mercati non anglofoni che la maggior parte degli strumenti dev ignora"]

---

## 7. RIEPILOGO STACK SOVRANO

### Cosa Posso Offrire Oggi
(In base a hardware + competenze + tempo, cosa potresti vendere QUESTA SETTIMANA se qualcuno lo chiedesse?)
1. [es., "Elaborazione documenti locale — estrazione dati da PDF in modo privato"]
2. [es., "Script di automazione personalizzati per [dominio specifico]"]
3. [es., "Scrittura tecnica / documentazione"]

### Cosa Sto Costruendo
(In base al framework STREETS completo — compila questo man mano che avanzi nel corso)
1. [Motore di Guadagno 1 — dal Modulo R]
2. [Motore di Guadagno 2 — dal Modulo R]
3. [Motore di Guadagno 3 — dal Modulo R]

### Vincoli Chiave
(Sii onesto — questi non sono debolezze, sono parametri)
- [es., "Solo 10 ore/settimana disponibili"]
- [es., "Nessuna GPU — inferenza solo CPU, mi appoggerò alle API per task LLM"]
- [es., "Il contratto di lavoro è restrittivo — devo restare in domini non correlati"]
- [es., "Non basato negli USA — alcune opzioni di pagamento/legali sono limitate"]

---

*Questo documento è un riferimento vivo. Aggiornalo mensilmente.*
*Prossima data di revisione: [Data + 30 giorni]*
```

{? if dna.primary_stack ?}
> **Pre-compilazione dal tuo Developer DNA:**
> - **Stack principale:** {= dna.primary_stack | fallback("Non rilevato") =}
> - **Interessi:** {= dna.interests | fallback("Non rilevati") =}
> - **Riepilogo identità:** {= dna.identity_summary | fallback("Non ancora profilato") =}
{? if dna.blind_spots ?}> - **Punti ciechi da osservare:** {= dna.blind_spots | fallback("Nessuno rilevato") =}
{? endif ?}
{? elif stack.primary ?}
> **Pre-compilazione dallo stack rilevato:** Le tue tecnologie principali sono {= stack.primary | fallback("non ancora rilevate") =}. {? if stack.adjacent ?}Competenze adiacenti: {= stack.adjacent | fallback("nessuna rilevata") =}.{? endif ?} Usale per compilare l'Inventario Competenze sopra.
{? endif ?}

{@ insight t_shape @}

### Come Usare Questo Documento

1. **Prima di iniziare qualsiasi nuovo progetto:** Controlla il tuo Stack Sovrano. Hai l'hardware, il tempo, le competenze e il budget per eseguire?
2. **Prima di comprare qualsiasi cosa:** Controlla la tua allocazione budget. Questo acquisto è nel piano?
3. **Revisione mensile:** Aggiorna la colonna "Effettivo" nel tuo budget. Aggiorna i numeri dei ricavi. Aggiusta le allocazioni in base a cosa funziona.
4. **Quando qualcuno chiede cosa fai:** La tua sezione "Cosa Posso Offrire Oggi" è il tuo pitch istantaneo.
5. **Quando sei tentato di inseguire una nuova idea brillante:** Controlla i tuoi vincoli. Rientra nel tuo tempo, competenze e hardware? Se no, aggiungilo a "Cosa Sto Costruendo" per dopo.

### L'Esercizio di Un'Ora

Imposta un timer per 60 minuti. Compila ogni campo del template. Non pensarci troppo. Non fare ricerche approfondite. Scrivi quello che sai adesso. Potrai aggiornarlo dopo.

I campi che non riesci a compilare? Quelli sono i tuoi action item per questa settimana:
- Numeri di benchmark vuoti? Esegui lo script di benchmark dalla Lezione 2.
- Nessuna entità aziendale? Inizia il processo di costituzione dalla Lezione 4.
- Nessuna elaborazione pagamenti? Configura Stripe dalla Lezione 4.
- Inventario competenze vuoto? Spendi 15 minuti elencando tutto per cui sei stato pagato negli ultimi 5 anni.

> **Errore Comune:** Passare 3 ore a rendere il documento "perfetto" invece di 1 ora a renderlo "fatto." Il Documento dello Stack Sovrano è un riferimento di lavoro, non un business plan per investitori. Nessuno lo vedrà tranne te. La precisione conta. La formattazione no.

### Checkpoint Lezione 6

Dovresti ora avere:
- [ ] Un Documento dello Stack Sovrano completo salvato da qualche parte che aprirai davvero
- [ ] Tutte e sei le sezioni compilate con numeri reali (non aspirazioanali)
- [ ] Una lista chiara di action item per le lacune nella tua configurazione
- [ ] Una data fissata per la tua prima revisione mensile (30 giorni da ora)

---

## Modulo S: Completato

{? if progress.completed("MODULE_S") ?}
> **Modulo S completato.** Hai finito {= progress.completed_count | fallback("1") =} di {= progress.total_count | fallback("7") =} moduli STREETS. {? if progress.completed_modules ?}Completati: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### Cosa Hai Costruito in Due Settimane

Guarda cosa hai adesso che non avevi quando hai iniziato:

1. **Un inventario hardware** mappato alle capacità di generazione di reddito — non solo specifiche su un adesivo.
2. **Uno stack LLM locale di livello produzione** con Ollama, testato sul tuo hardware reale, configurato per carichi di lavoro reali.
3. **Un vantaggio sulla privacy** che sai come commercializzare — con linguaggio specifico per audience specifiche.
4. **Una base legale e finanziaria** — entità aziendale (o piano), elaborazione pagamenti, conto bancario, strategia fiscale.
5. **Un budget controllato** con obiettivi ROI chiari e una scadenza di 90 giorni per provare il modello.
6. **Un Documento dello Stack Sovrano** che cattura tutto quanto sopra in un unico riferimento che userai per ogni decisione futura.

Questo è più di quanto la maggior parte degli sviluppatori configuri mai. Sul serio. La maggior parte delle persone che vuole generare reddito extra salta direttamente a "costruire qualcosa di figo" e poi si chiede perché non riesce a farsi pagare. Ora hai l'infrastruttura per farti pagare.

Ma l'infrastruttura senza direzione è solo un hobby costoso. Devi sapere dove puntare questo stack.

{@ temporal market_timing @}

### Cosa Viene Dopo: Modulo T — Fossati Tecnici

Il Modulo S ti ha dato le fondamenta. Il Modulo T risponde alla domanda critica: **come costruisci qualcosa che i concorrenti non possono facilmente copiare?**

Ecco cosa copre il Modulo T:

- **Pipeline di dati proprietarie** — come creare dataset a cui solo tu hai accesso, legalmente ed eticamente
- **Configurazioni modello personalizzate** — fine-tuning e prompt engineering che producono qualità di output che altri non possono eguagliare con le impostazioni di default
- **Stack di competenze che compongono** — perché "Python + sanità" batte "Python + JavaScript" per il reddito, e come identificare la tua combinazione unica
- **Barriere tecniche all'ingresso** — design infrastrutturali che richiederebbero mesi a un concorrente per replicare
- **L'Audit del Fossato** — un framework per valutare se il tuo progetto ha un vantaggio difendibile o è solo un altro servizio commodity

La differenza tra uno sviluppatore che fa $500/mese e uno che fa $5.000/mese è raramente la competenza. Sono i fossati. Cose che rendono la tua offerta difficile da replicare, anche se qualcuno ha lo stesso hardware e gli stessi modelli.

### La Roadmap STREETS Completa

| Modulo | Titolo | Focus | Durata |
|--------|--------|-------|--------|
| **S** | Configurazione Sovrana | Infrastruttura, legale, budget | Settimane 1-2 (completato) |
| **T** | Fossati Tecnici | Vantaggi difendibili, asset proprietari | Settimane 3-4 |
| **R** | Motori di Guadagno | Playbook di monetizzazione specifici con codice | Settimane 5-8 |
| **E** | Playbook di Esecuzione | Sequenze di lancio, pricing, primi clienti | Settimane 9-10 |
| **E** | Vantaggio in Evoluzione | Restare avanti, rilevamento trend, adattamento | Settimane 11-12 |
| **T** | Automazione Tattica | Automatizzare le operazioni per reddito passivo | Settimane 13-14 |
| **S** | Sovrapposizione Flussi | Fonti di reddito multiple, strategia di portfolio | Settimane 15-16 |

Il Modulo R (Motori di Guadagno) è dove si guadagna la maggior parte dei soldi. Ma senza S e T, stai costruendo sulla sabbia.

---

**Pronto per il playbook completo?**

Hai visto le fondamenta. Le hai costruite tu stesso. Ora ottieni il sistema completo.

**Ottieni STREETS Core** — il corso completo di 16 settimane con tutti e sette i moduli, template di codice per motori di guadagno, dashboard finanziarie e la community privata di sviluppatori che costruiscono reddito alle proprie condizioni.
