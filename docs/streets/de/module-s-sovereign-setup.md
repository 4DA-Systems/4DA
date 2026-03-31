# Modul S: Souveränes Setup

**STREETS Developer-Einkommenskurs — Kostenloses Modul**
*Wochen 1-2 | 6 Lektionen | Ergebnis: Dein Sovereign Stack Dokument*

> "Dein Rechner ist deine Geschäftsinfrastruktur. Konfiguriere ihn entsprechend."

---

Du besitzt bereits das mächtigste einkommensgenerierende Werkzeug, das die meisten Menschen nie haben werden: eine Entwickler-Workstation mit Internetverbindung, lokaler Rechenleistung und den Fähigkeiten, alles zusammenzuschalten.

Die meisten Entwickler behandeln ihren Rechner wie ein Konsumprodukt. Etwas, worauf sie spielen, coden, surfen. Aber dieselbe Maschine — die jetzt gerade unter deinem Schreibtisch steht — kann Inferenz ausführen, APIs bereitstellen, Daten verarbeiten und 24 Stunden am Tag Umsatz generieren, während du schläfst.

In diesem Modul geht es darum, das, was du bereits hast, durch eine andere Brille zu betrachten. Nicht "Was kann ich bauen?" sondern "Was kann ich verkaufen?"

Am Ende dieser zwei Wochen wirst du haben:

- Ein klares Inventar deiner einkommensgenerierenden Fähigkeiten
- Einen produktionsreifen lokalen LLM-Stack
- Ein rechtliches und finanzielles Fundament (auch wenn minimal)
- Ein schriftliches Sovereign Stack Dokument, das dein Geschäftsplan wird

Kein Herumgerede. Kein "Glaub einfach an dich." Echte Zahlen, echte Befehle, echte Entscheidungen.

{@ mirror sovereign_readiness @}

Los geht's.

---

## Lektion 1: Das Rig-Audit

*"Du brauchst keine 4090. Hier ist, was wirklich zählt."*

### Dein Rechner ist ein Geschäftswert

Wenn ein Unternehmen seine Infrastruktur bewertet, listet es nicht einfach Spezifikationen auf — es ordnet Fähigkeiten den Umsatzmöglichkeiten zu. Genau das wirst du jetzt tun.

{? if computed.profile_completeness != "0" ?}
> **Dein aktueller Rechner:** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} Kerne / {= profile.cpu.threads | fallback("?") =} Threads), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} frei / {= profile.storage.total | fallback("?") =} gesamt ({= profile.storage.type | fallback("unknown") =}), läuft auf {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =}.
{? endif ?}

Öffne ein Terminal und gehe die folgenden Punkte durch. Schreibe jede Zahl auf. Du wirst sie für dein Sovereign Stack Dokument in Lektion 6 brauchen.

### Hardware-Inventar

#### CPU

```bash
# Linux/Mac
lscpu | grep "Model name\|CPU(s)\|Thread(s)"
# or
cat /proc/cpuinfo | grep "model name" | head -1
nproc

# Windows (PowerShell)
Get-CimInstance -ClassName Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors

# macOS
sysctl -n machdep.cpu.brand_string
sysctl -n hw.ncpu
```

**Was für das Einkommen wichtig ist:**
- Die Kernanzahl bestimmt, wie viele gleichzeitige Aufgaben dein Rechner bewältigen kann. Ein lokales LLM laufen zu lassen und gleichzeitig einen Batch-Job zu verarbeiten, erfordert echte Parallelität.
{? if profile.cpu.cores ?}
- *Dein {= profile.cpu.model | fallback("CPU") =} hat {= profile.cpu.cores | fallback("?") =} Kerne — prüfe die Anforderungstabelle unten, um zu sehen, welche Revenue Engines deine CPU unterstützt.*
{? endif ?}
- Für die meisten Revenue Engines in diesem Kurs reicht jede moderne 8+-Kern-CPU der letzten 5 Jahre.
- Wenn du lokale LLMs nur auf der CPU laufen lässt (keine GPU), willst du 16+ Kerne. Ein Ryzen 7 5800X oder Intel i7-12700 ist die praktische Untergrenze.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**Was für das Einkommen wichtig ist:**
- 16 GB: Absolutes Minimum. Du kannst 7B-Modelle laufen lassen und grundlegende Automatisierungsarbeit machen.
- 32 GB: Komfortabel. 13B-Modelle lokal laufen lassen, mehrere Projekte handhaben, deine Entwicklungsumgebung neben Einkommens-Workloads aktiv halten.
- 64 GB+: Du kannst 30B+-Modelle auf der CPU laufen lassen oder mehrere Modelle gleichzeitig geladen halten. Hier wird es interessant für den Verkauf von Inferenz-Diensten.
{? if profile.ram.total ?}
*Dein System hat {= profile.ram.total | fallback("?") =} RAM. Prüfe die obige Tabelle, um zu sehen, in welcher Leistungsstufe du bist — das beeinflusst direkt, welche lokalen Modelle für deine Einkommens-Workloads praktikabel sind.*
{? endif ?}

#### GPU

```bash
# NVIDIA
nvidia-smi

# Check VRAM specifically
nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv

# AMD (Linux)
rocm-smi

# macOS (Apple Silicon)
system_profiler SPDisplaysDataType
```

**Was für das Einkommen wichtig ist:**

Das ist die eine Spezifikation, über die alle obsessiv reden, und hier ist die ehrliche Wahrheit: **Deine GPU bestimmt deine lokale LLM-Stufe, und deine lokale LLM-Stufe bestimmt, welche Einkommensströme am schnellsten laufen.** Aber sie bestimmt nicht, ob du überhaupt Geld verdienen kannst.

| VRAM | LLM-Fähigkeit | Einkommensrelevanz |
|------|---------------|------------------|
| 0 (nur CPU) | 7B-Modelle bei ~5 Tokens/Sek. | Batch-Verarbeitung, asynchrone Arbeit. Langsam, aber funktional. |
| 6-8 GB (RTX 3060 usw.) | 7B-Modelle bei ~30 Tok/Sek., 13B quantisiert | Gut genug für die meisten Automatisierungs-Einkommensströme. |
| 12 GB (RTX 3060 12GB, 4070) | 13B mit voller Geschwindigkeit, 30B quantisiert | Sweet Spot. Die meisten Revenue Engines laufen hier gut. |
| 16-24 GB (RTX 4090, 3090) | 30B-70B-Modelle | Premium-Stufe. Verkaufe Qualität, die andere lokal nicht erreichen können. |
| 48 GB+ (Dual-GPU, A6000) | 70B+ mit Geschwindigkeit | Enterprise-Grade lokale Inferenz. Ernsthafter Wettbewerbsvorteil. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ mit Unified Memory | Hervorragende Effizienz. Niedrigere Stromkosten als NVIDIA-Äquivalent. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **Deine GPU:** {= profile.gpu.model | fallback("Unknown") =} mit {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}du bist in der Premium-Stufe. 30B-70B-Modelle sind lokal erreichbar. Das ist ein ernsthafter Wettbewerbsvorteil.{? elif computed.gpu_tier == "sweet_spot" ?}du bist im Sweet Spot. 13B mit voller Geschwindigkeit, 30B quantisiert. Die meisten Revenue Engines laufen hier gut.{? elif computed.gpu_tier == "capable" ?}du kannst 7B-Modelle mit guter Geschwindigkeit und 13B quantisiert laufen lassen. Gut genug für die meisten Automatisierungs-Einkommensströme.{? else ?}du hast GPU-Beschleunigung verfügbar. Prüfe die obige Tabelle, um zu sehen, wo du stehst.{? endif ?}
{? else ?}
> **Keine dedizierte GPU erkannt.** Du wirst Inferenz auf der CPU ausführen, was ~5-12 Tokens/Sek. bei 7B-Modellen bedeutet. Das ist in Ordnung für Batch-Verarbeitung und asynchrone Arbeit. Nutze API-Aufrufe, um die Geschwindigkeitslücke bei kundenorientierter Ausgabe zu schließen.
{? endif ?}

> **Klartext:** Wenn du eine RTX 3060 12GB hast, bist du in einer besseren Position als 95% der Entwickler, die versuchen, mit KI Geld zu verdienen. Hör auf, auf eine 4090 zu warten. Die 3060 12GB ist der Honda Civic der lokalen KI — zuverlässig, effizient, erledigt den Job. Das Geld, das du für ein GPU-Upgrade ausgeben würdest, ist besser in API-Credits für kundenorientierte Qualität investiert, während deine lokalen Modelle die Routinearbeit übernehmen.

#### Speicher

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**Was für das Einkommen wichtig ist:**
- LLM-Modelle brauchen Platz: 7B-Modell = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (quantisiert).
- Du brauchst Platz für Projektdaten, Datenbanken, Caches und Ausgabe-Artefakte.
- SSD ist nicht verhandelbar für alles, was kundenorientiert ist. Modell-Laden von HDD fügt 30-60 Sekunden Startzeit hinzu.
- Praktisches Minimum: 500 GB SSD mit mindestens 100 GB frei.
- Komfortabel: 1 TB SSD. Modelle auf der SSD halten, auf HDD archivieren.
{? if profile.storage.free ?}
*Du hast {= profile.storage.free | fallback("?") =} frei auf {= profile.storage.type | fallback("your drive") =}. {? if profile.storage.type == "SSD" ?}Gut — SSD bedeutet schnelles Modell-Laden.{? elif profile.storage.type == "NVMe" ?}Hervorragend — NVMe ist die schnellste Option fürs Modell-Laden.{? else ?}Erwäge eine SSD, falls du nicht bereits eine nutzt — es macht einen echten Unterschied bei den Modell-Ladezeiten.{? endif ?}*
{? endif ?}

#### Netzwerk

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**Was für das Einkommen wichtig ist:**
{? if profile.network.download ?}
*Deine Verbindung: {= profile.network.download | fallback("?") =} Download / {= profile.network.upload | fallback("?") =} Upload.*
{? endif ?}
- Download-Geschwindigkeit: 50+ Mbps. Nötig für das Herunterladen von Modellen, Paketen und Daten.
- Upload-Geschwindigkeit: Das ist der Engpass, den die meisten ignorieren. Wenn du irgendetwas bereitstellst (APIs, verarbeitete Ergebnisse, Liefergegenstände), ist Upload wichtig.
  - 10 Mbps: Ausreichend für asynchrone Lieferung (verarbeitete Dateien, Batch-Ergebnisse).
  - 50+ Mbps: Erforderlich, wenn du irgendeine Art von lokalem API-Endpunkt betreibst, den externe Dienste ansprechen.
  - 100+ Mbps: Komfortabel für alles in diesem Kurs.
- Latenz: Unter 50ms zu großen Cloud-Anbietern. Führe `ping api.openai.com` und `ping api.anthropic.com` aus, um zu prüfen.

#### Verfügbarkeit

Das ist die Spezifikation, an die niemand denkt, aber sie trennt Hobbyisten von Leuten, die Geld im Schlaf verdienen.

Frag dich selbst:
- Kann dein Rechner 24/7 laufen? (Strom, Kühlung, Lautstärke)
- Hast du eine USV für Stromausfälle?
- Ist deine Internetverbindung stabil genug für automatisierte Workflows?
- Kannst du dich per SSH auf deinen Rechner verbinden, wenn etwas kaputtgeht?

Wenn du nicht 24/7 laufen kannst, ist das in Ordnung — viele Einkommensströme in diesem Kurs sind asynchrone Batch-Jobs, die du manuell auslöst. Aber diejenigen, die wirklich passives Einkommen generieren, erfordern Verfügbarkeit.

{? if computed.os_family == "windows" ?}
**Schnelles Verfügbarkeits-Setup (Windows):** Nutze den Taskplaner für automatischen Neustart, aktiviere Remote Desktop oder installiere Tailscale für Fernzugriff, und konfiguriere dein BIOS für "Bei Stromausfall wiederherstellen", um nach Ausfällen wiederherzustellen.
{? endif ?}

**Schnelles Verfügbarkeits-Setup (falls gewünscht):**

```bash
# Enable Wake-on-LAN (check BIOS)
# Set up SSH access
sudo systemctl enable ssh  # Linux

# Auto-restart on crash (systemd service example)
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

### Die Stromkosten-Rechnung

Die Leute ignorieren das entweder oder machen eine Katastrophe daraus. Rechnen wir mal richtig.

**Deinen tatsächlichen Stromverbrauch messen:**

```bash
# If you have a Kill-A-Watt meter or smart plug with monitoring:
# Measure at idle, at load (running inference), and at max (GPU full utilization)

# Rough estimates if you don't have a meter:
# Desktop (no GPU, idle): 60-100W
# Desktop (mid-range GPU, idle): 80-130W
# Desktop (high-end GPU, idle): 100-180W
# Desktop (GPU under inference load): add 50-80% of GPU TDP
# Laptop: 15-45W
# Mac Mini M2: 7-15W (seriously)
# Apple Silicon laptop: 10-30W
```

**Monatliche Kostenberechnung:**

```
Monthly cost = (Watts / 1000) x Hours x Price per kWh

Example: Desktop with RTX 3060, running inference 8 hours/day, idle 16 hours/day
- Inference: (250W / 1000) x 8h x 30 days x $0.12/kWh = $7.20/month
- Idle: (100W / 1000) x 16h x 30 days x $0.12/kWh = $5.76/month
- Total: ~$13/month

Example: Same rig, 24/7 inference
- (250W / 1000) x 24h x 30 days x $0.12/kWh = $21.60/month

Example: Mac Mini M2, 24/7
- (12W / 1000) x 24h x 30 days x $0.12/kWh = $1.04/month
```

{? if regional.country ?}
Dein Strompreis: ungefähr {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh (basierend auf Durchschnittswerten für {= regional.country | fallback("your region") =}). Prüfe deine tatsächliche Stromrechnung — die Tarife variieren je nach Anbieter und Tageszeit.
{? else ?}
Der US-Durchschnittsstrompreis liegt bei etwa $0,12/kWh. Prüfe deinen tatsächlichen Tarif — er variiert stark. Kalifornien kann $0,25/kWh betragen. Einige europäische Länder erreichen $0,35/kWh. Teile des US-Mittleren Westens liegen bei $0,08/kWh.
{? endif ?}

**Der Punkt:** Deinen Rechner 24/7 für Einkommen laufen zu lassen, kostet irgendwo zwischen {= regional.currency_symbol | fallback("$") =}1-{= regional.currency_symbol | fallback("$") =}30/Monat an Strom. Wenn deine Einkommensströme das nicht decken können, liegt das Problem nicht am Strom — sondern am Einkommensstrom.

### Mindestanforderungen nach Revenue-Engine-Typ

Hier ist eine Vorschau, wohin wir im vollständigen STREETS-Kurs gehen. Prüfe zunächst einfach, wo dein Rechner steht:

| Revenue Engine | CPU | RAM | GPU | Speicher | Netzwerk |
|---------------|-----|-----|-----|---------|---------|
| **Content-Automatisierung** (Blogbeiträge, Newsletter) | 4+ Kerne | 16 GB | Optional (API-Fallback) | 50 GB frei | 10 Mbps Upload |
| **Datenverarbeitungs-Dienste** | 8+ Kerne | 32 GB | Optional | 200 GB frei | 50 Mbps Upload |
| **Lokale KI-API-Dienste** | 8+ Kerne | 32 GB | 8+ GB VRAM | 100 GB frei | 50 Mbps Upload |
| **Code-Generierungs-Tools** | 8+ Kerne | 16 GB | 8+ GB VRAM oder API | 50 GB frei | 10 Mbps Upload |
| **Dokumentenverarbeitung** | 4+ Kerne | 16 GB | Optional | 100 GB frei | 10 Mbps Upload |
| **Autonome Agenten** | 8+ Kerne | 32 GB | 12+ GB VRAM | 100 GB frei | 50 Mbps Upload |

> **Häufiger Fehler:** "Ich muss erst meine Hardware aufrüsten, bevor ich anfangen kann." Nein. Fang mit dem an, was du hast. Nutze API-Aufrufe, um Lücken zu füllen, die deine Hardware nicht abdecken kann. Rüste auf, wenn der Umsatz es rechtfertigt — nicht vorher.

{@ insight engine_ranking @}

### Lektion 1 Checkpoint

Du solltest jetzt aufgeschrieben haben:
- [ ] CPU-Modell, Kerne und Threads
- [ ] RAM-Menge
- [ ] GPU-Modell und VRAM (oder "keine")
- [ ] Verfügbarer Speicherplatz
- [ ] Netzwerkgeschwindigkeiten (Download/Upload)
- [ ] Geschätzte monatliche Stromkosten für 24/7-Betrieb
- [ ] Für welche Revenue-Engine-Kategorien dein Rechner qualifiziert ist

Bewahre diese Zahlen auf. Du wirst sie in Lektion 6 in dein Sovereign Stack Dokument eintragen.

{? if computed.profile_completeness != "0" ?}
> **4DA hat die meisten dieser Zahlen bereits für dich erfasst.** Prüfe die personalisierten Zusammenfassungen oben — dein Hardware-Inventar ist teilweise aus der Systemerkennung vorausgefüllt.
{? endif ?}

*Im vollständigen STREETS-Kurs gibt dir Modul R (Revenue Engines) spezifische, Schritt-für-Schritt-Anleitungen für jeden oben aufgeführten Engine-Typ — einschließlich des exakten Codes zum Bauen und Bereitstellen.*

---

## Lektion 2: Der lokale LLM-Stack

*"Richte Ollama für den Produktiveinsatz ein — nicht nur zum Chatten."*

### Warum lokale LLMs für das Einkommen wichtig sind

Jedes Mal, wenn du die OpenAI API aufrufst, zahlst du Miete. Jedes Mal, wenn du ein Modell lokal ausführst, ist diese Inferenz nach dem initialen Setup kostenlos. Die Rechnung ist einfach:

- GPT-4o: ~$5 pro Million Input-Tokens, ~$15 pro Million Output-Tokens
- Claude 3.5 Sonnet: ~$3 pro Million Input-Tokens, ~$15 pro Million Output-Tokens
- Lokales Llama 3.1 8B: $0 pro Million Tokens (nur Stromkosten)

Wenn du Dienste baust, die Tausende von Anfragen verarbeiten, ist der Unterschied zwischen $0 und $5-$15 pro Million Tokens der Unterschied zwischen Gewinn und Break-even.

Aber hier ist die Nuance, die die meisten übersehen: **Lokale und API-Modelle erfüllen unterschiedliche Rollen in einem Einkommens-Stack.** Lokale Modelle bewältigen Volumen. API-Modelle bewältigen qualitätskritische, kundenorientierte Ausgaben. Dein Stack braucht beides.

### Ollama installieren

{? if settings.has_llm ?}
> **Du hast bereits ein LLM konfiguriert:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}. Wenn Ollama bereits läuft, springe direkt zu "Modell-Auswahlhilfe" weiter unten.
{? endif ?}

Ollama ist das Fundament. Es verwandelt deinen Rechner in einen lokalen Inferenz-Server mit einer sauberen API.

```bash
# Linux
curl -fsSL https://ollama.com/install.sh | sh

# macOS
# Download from https://ollama.com or:
brew install ollama

# Windows
# Download installer from https://ollama.com
# Or use winget:
winget install Ollama.Ollama
```

{? if computed.os_family == "windows" ?}
> **Windows:** Nutze den Installer von ollama.com oder `winget install Ollama.Ollama`. Ollama läuft nach der Installation automatisch als Hintergrunddienst.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` ist der schnellste Weg. Ollama nutzt das Unified Memory von Apple Silicon — dein {= profile.ram.total | fallback("system") =} RAM wird zwischen CPU- und GPU-Workloads geteilt.
{? elif computed.os_family == "linux" ?}
> **Linux:** Das Installationsskript erledigt alles. Wenn du {= profile.os.name | fallback("Linux") =} verwendest, wird Ollama als systemd-Dienst installiert.
{? endif ?}

Überprüfe die Installation:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **Versionshinweis:** Ollama veröffentlicht häufig Updates. Die Modell-Befehle und Flags in diesem Modul wurden mit Ollama v0.5.x (Anfang 2026) überprüft. Wenn du das später liest, prüfe [ollama.com/download](https://ollama.com/download) für die neueste Version und [ollama.com/library](https://ollama.com/library) für aktuelle Modellnamen. Die Kernkonzepte ändern sich nicht, aber spezifische Modell-Tags (z.B. `llama3.1:8b`) können durch neuere Releases ersetzt werden.

### Modell-Auswahlhilfe

Lade nicht jedes Modell herunter, das du siehst. Sei strategisch. Hier ist, was du herunterladen solltest und wann du was verwendest.

{? if computed.llm_tier ?}
> **Deine LLM-Stufe (basierend auf Hardware):** {= computed.llm_tier | fallback("unknown") =}. Die Empfehlungen unten sind markiert, damit du dich auf die Stufe konzentrieren kannst, die zu deinem Rechner passt.
{? endif ?}

#### Stufe 1: Das Arbeitstier (7B-8B-Modelle)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**Verwende für:**
- Textklassifizierung ("Ist diese E-Mail Spam oder seriös?")
- Zusammenfassungen (lange Dokumente in Stichpunkte kondensieren)
- Einfache Datenextraktion (Namen, Daten, Beträge aus Text extrahieren)
- Sentimentanalyse
- Content-Tagging und Kategorisierung
- Embedding-Generierung (wenn ein Modell mit Embedding-Unterstützung verwendet wird)

**Leistung (typisch):**
- RTX 3060 12GB: ~40-60 Tokens/Sekunde
- RTX 4090: ~100-130 Tokens/Sekunde
- M2 Pro 16GB: ~30-45 Tokens/Sekunde
- Nur CPU (Ryzen 7 5800X): ~8-12 Tokens/Sekunde

**Kostenvergleich:**
- 1 Million Tokens via GPT-4o-mini: ~$0,60
- 1 Million Tokens lokal (8B-Modell): ~$0,003 an Stromkosten
- Break-even-Punkt: ~5.000 Tokens (du sparst buchstäblich ab der ersten Anfrage Geld)

#### Stufe 2: Die ausgewogene Wahl (13B-14B-Modelle)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**Verwende für:**
- Content-Entwürfe (Blogbeiträge, Dokumentation, Marketingtexte)
- Code-Generierung (Funktionen, Skripte, Boilerplate)
- Komplexe Datentransformation
- Mehrstufige Reasoning-Aufgaben
- Übersetzung mit Nuancen

**Leistung (typisch):**
- RTX 3060 12GB: ~20-30 Tokens/Sekunde (quantisiert)
- RTX 4090: ~60-80 Tokens/Sekunde
- M2 Pro 32GB: ~20-30 Tokens/Sekunde
- Nur CPU: ~3-6 Tokens/Sekunde (nicht praktikabel für Echtzeit)

**Wann statt 7B verwenden:** Wenn die Ausgabequalität von 7B nicht gut genug ist, du aber nicht für API-Aufrufe zahlen willst. Teste beides an deinem tatsächlichen Anwendungsfall — manchmal reicht 7B und du verschwendest nur Rechenleistung.

{? if computed.gpu_tier == "capable" ?}
> **Stufe-3-Streckgebiet** — Deine {= profile.gpu.model | fallback("GPU") =} kann 30B quantisiert mit etwas Aufwand bewältigen, aber 70B ist lokal nicht erreichbar. Erwäge API-Aufrufe für Aufgaben, die 70B-Level-Qualität erfordern.
{? endif ?}

#### Stufe 3: Die Qualitätsstufe (30B-70B-Modelle)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**Verwende für:**
- Kundenorientierte Inhalte, die exzellent sein müssen
- Komplexe Analyse und Reasoning
- Langform-Content-Generierung
- Aufgaben, bei denen Qualität direkt beeinflusst, ob jemand zahlt

**Leistung (typisch):**
- RTX 4090 (24GB): 70B bei ~8-15 Tokens/Sekunde (nutzbar, aber langsam)
- Dual-GPU oder 48GB+: 70B bei ~20-30 Tokens/Sekunde
- M3 Max 64GB: 70B bei ~10-15 Tokens/Sekunde

> **Klartext:** Wenn du keine 24GB+ VRAM hast, überspringe die 70B-Modelle komplett. Nutze API-Aufrufe für qualitätskritische Ausgaben. Ein 70B-Modell mit 3 Tokens/Sekunde aus dem System-RAM laufen zu lassen, ist technisch möglich, aber praktisch nutzlos für jeden einkommensgenerierenden Workflow. Deine Zeit hat einen Wert.

#### Stufe 4: API-Modelle (Wenn lokal nicht reicht)

Lokale Modelle sind für Volumen und Privatsphäre. API-Modelle sind für Qualitätsobergrenzen und spezialisierte Fähigkeiten.

**Wann API-Modelle verwenden:**
- Kundenorientierte Ausgaben, bei denen Qualität = Umsatz (Verkaufstexte, Premium-Inhalte)
- Komplexe Reasoning-Ketten, bei denen kleinere Modelle versagen
- Vision/Multimodal-Aufgaben (Bilder, Screenshots, Dokumente analysieren)
- Wenn du strukturierte JSON-Ausgaben mit hoher Zuverlässigkeit brauchst
- Wenn Geschwindigkeit wichtig ist und deine lokale Hardware langsam ist

**Kostenvergleichstabelle (Stand Anfang 2025 — aktuelle Preise prüfen):**

| Modell | Input (pro 1M Tokens) | Output (pro 1M Tokens) | Am besten für |
|-------|----------------------|------------------------|----------|
| GPT-4o-mini | $0,15 | $0,60 | Günstige Volumenarbeit (wenn lokal nicht verfügbar) |
| GPT-4o | $2,50 | $10,00 | Vision, komplexes Reasoning |
| Claude 3.5 Sonnet | $3,00 | $15,00 | Code, Analyse, langer Kontext |
| Claude 3.5 Haiku | $0,80 | $4,00 | Schnell, günstig, gutes Qualitätsgleichgewicht |
| DeepSeek V3 | $0,27 | $1,10 | Budget-freundlich, starke Leistung |

**Die Hybrid-Strategie:**
1. Lokale 7B/13B bewältigen 80% der Anfragen (Klassifizierung, Extraktion, Zusammenfassung)
2. API bewältigt 20% der Anfragen (finale Qualitätsprüfung, komplexe Aufgaben)
3. Deine effektiven Kosten: ~$0,50-2,00 pro Million Tokens gemischt (statt $5-15 reine API)

Dieser Hybrid-Ansatz ist der Weg, wie du Dienste mit gesunden Margen baust. Mehr dazu in Modul R.

### Produktionskonfiguration

Ollama für Einkommensarbeit zu betreiben, ist anders als es für persönlichen Chat zu nutzen. Hier ist, wie du es richtig konfigurierst.

{? if computed.has_nvidia ?}
> **NVIDIA GPU erkannt ({= profile.gpu.model | fallback("unknown") =}).** Ollama wird automatisch CUDA-Beschleunigung nutzen. Stelle sicher, dass deine NVIDIA-Treiber aktuell sind — führe `nvidia-smi` aus, um zu prüfen. Für optimale Leistung mit {= profile.gpu.vram | fallback("your") =} VRAM sollte die `OLLAMA_MAX_LOADED_MODELS`-Einstellung unten dazu passen, wie viele Modelle gleichzeitig in deinen VRAM passen.
{? endif ?}

#### Umgebungsvariablen setzen

```bash
# Create/edit the Ollama configuration
# Linux: /etc/systemd/system/ollama.service or environment variables
# macOS: launchctl environment or ~/.zshrc
# Windows: System Environment Variables

# Key settings:
export OLLAMA_HOST=127.0.0.1:11434    # Bind to localhost only (security)
export OLLAMA_NUM_PARALLEL=4            # Concurrent request handling
export OLLAMA_MAX_LOADED_MODELS=2       # Keep 2 models in memory
export OLLAMA_KEEP_ALIVE=30m            # Keep model loaded for 30 min after last request
export OLLAMA_MAX_QUEUE=100             # Queue up to 100 requests
```

#### Ein Modelfile für deinen Workload erstellen

Anstatt Standard-Modelleinstellungen zu verwenden, erstelle ein benutzerdefiniertes Modelfile, das auf deinen Einkommens-Workload abgestimmt ist:

```dockerfile
# Save as: Modelfile-worker
FROM llama3.1:8b

# Tune for consistent, production output
PARAMETER temperature 0.3
PARAMETER top_p 0.9
PARAMETER num_ctx 4096
PARAMETER repeat_penalty 1.1

# System prompt for your most common workload
SYSTEM """You are a precise data processing assistant. You follow instructions exactly. You output only what is requested, with no preamble or explanation unless asked. When given structured output formats (JSON, CSV, etc.), you output only the structure with no markdown formatting."""
```

```bash
# Create your custom model
ollama create worker -f Modelfile-worker

# Test it
ollama run worker "Extract all email addresses from this text: Contact us at hello@example.com or support@test.org for more info."
```

#### Batching und Warteschlangenverwaltung

Für Einkommens-Workloads musst du oft viele Elemente verarbeiten. Hier ist ein einfaches Batching-Setup:

```python
#!/usr/bin/env python3
"""
batch_processor.py — Process items through local LLM with queuing.
Production-grade batching for income workloads.
"""

import requests
import json
import time
import concurrent.futures
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "worker"  # Your custom model from above
MAX_CONCURRENT = 4
MAX_RETRIES = 3

def process_item(item: dict) -> dict:
    """Process a single item through the local LLM."""
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
            time.sleep(2 ** attempt)  # Exponential backoff

def process_batch(items: list[dict], output_file: str = "results.jsonl"):
    """Process a batch of items with concurrent execution."""
    results = []
    start_time = time.time()

    with concurrent.futures.ThreadPoolExecutor(max_workers=MAX_CONCURRENT) as executor:
        future_to_item = {executor.submit(process_item, item): item for item in items}

        for i, future in enumerate(concurrent.futures.as_completed(future_to_item)):
            result = future.result()
            results.append(result)

            # Write incrementally (don't lose progress on crash)
            with open(output_file, "a") as f:
                f.write(json.dumps(result) + "\n")

            # Progress reporting
            elapsed = time.time() - start_time
            rate = (i + 1) / elapsed
            remaining = (len(items) - i - 1) / rate if rate > 0 else 0
            print(f"[{i+1}/{len(items)}] {result['status']} | "
                  f"{rate:.1f} items/sec | "
                  f"ETA: {remaining:.0f}s")

    # Summary
    succeeded = sum(1 for r in results if r["status"] == "success")
    failed = sum(1 for r in results if r["status"] == "failed")
    total_time = time.time() - start_time

    print(f"\nBatch complete: {succeeded} succeeded, {failed} failed, "
          f"{total_time:.1f}s total")

    return results

# Example usage:
if __name__ == "__main__":
    # Your items to process
    items = [
        {"id": i, "prompt": f"Summarize this in one sentence: {text}"}
        for i, text in enumerate(load_your_data())  # Replace with your data source
    ]

    results = process_batch(items)
```

### Benchmarking DEINES Rechners

Vertraue nicht den Benchmarks anderer. Miss deine eigenen:

```bash
# Quick benchmark script
# Save as: benchmark.sh

#!/bin/bash
MODELS=("llama3.1:8b" "mistral:7b")
PROMPT="Write a detailed 200-word product description for a wireless mechanical keyboard designed for programmers."

for model in "${MODELS[@]}"; do
    echo "=== Benchmarking: $model ==="

    # Warm up (first run loads model into memory)
    ollama run "$model" "Hello" > /dev/null 2>&1

    # Timed run
    START=$(date +%s%N)
    RESULT=$(curl -s http://localhost:11434/api/generate -d "{
        \"model\": \"$model\",
        \"prompt\": \"$PROMPT\",
        \"stream\": false
    }")
    END=$(date +%s%N)

    DURATION=$(( (END - START) / 1000000 ))
    TOKENS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('eval_count', 'N/A'))")

    echo "Time: ${DURATION}ms"
    echo "Tokens generated: $TOKENS"
    if [ "$TOKENS" != "N/A" ] && [ "$DURATION" -gt 0 ]; then
        TPS=$(python3 -c "print(f'{$TOKENS / ($DURATION / 1000):.1f}')")
        echo "Speed: $TPS tokens/second"
    fi
    echo ""
done
```

```bash
chmod +x benchmark.sh
./benchmark.sh
```

Schreibe deine Tokens/Sekunde für jedes Modell auf. Diese Zahl bestimmt, welche Einkommens-Workflows für deinen Rechner praktikabel sind.

{@ insight stack_fit @}

**Geschwindigkeitsanforderungen nach Anwendungsfall:**
- Batch-Verarbeitung (asynchron): 5+ Tokens/Sek. ist in Ordnung (Latenz ist egal)
- Interaktive Tools (Benutzer wartet): Minimum 20+ Tokens/Sek.
- Echtzeit-API (kundenorientiert): 30+ Tokens/Sek. für gute UX
- Streaming-Chat: 15+ Tokens/Sek. fühlt sich reaktionsschnell an

### Deinen lokalen Inferenz-Server absichern

{? if computed.os_family == "windows" ?}
> **Windows-Hinweis:** Ollama auf Windows bindet standardmäßig an localhost. Überprüfe mit `netstat -an | findstr 11434` in PowerShell. Nutze die Windows-Firewall, um externen Zugriff auf Port 11434 zu blockieren.
{? elif computed.os_family == "macos" ?}
> **macOS-Hinweis:** Ollama auf macOS bindet standardmäßig an localhost. Überprüfe mit `lsof -i :11434`. Die macOS-Firewall sollte externe Verbindungen automatisch blockieren.
{? endif ?}

Deine Ollama-Instanz sollte niemals aus dem Internet erreichbar sein, es sei denn, du beabsichtigst es ausdrücklich.

```bash
# Verify Ollama is only listening on localhost
ss -tlnp | grep 11434
# Should show 127.0.0.1:11434, NOT 0.0.0.0:11434

# If you need remote access (e.g., from another machine on your LAN):
# Use SSH tunneling instead of exposing the port
ssh -L 11434:localhost:11434 your-rig-ip

# Firewall rules (Linux)
sudo ufw deny in 11434
sudo ufw allow from 192.168.1.0/24 to any port 11434  # LAN only, if needed
```

> **Häufiger Fehler:** Ollama zur "Bequemlichkeit" an 0.0.0.0 binden und es dann vergessen. Jeder, der deine IP findet, kann deine GPU für kostenlose Inferenz nutzen. Schlimmer noch: Er kann Modellgewichte und System-Prompts extrahieren. Immer localhost. Immer Tunnel.

### Lektion 2 Checkpoint

Du solltest jetzt haben:
- [ ] Ollama installiert und laufend
- [ ] Mindestens ein Arbeitstier-Modell heruntergeladen (llama3.1:8b oder Äquivalent)
- [ ] Ein benutzerdefiniertes Modelfile für deinen erwarteten Workload
- [ ] Benchmark-Zahlen: Tokens/Sekunde für jedes Modell auf deinem Rechner
- [ ] Ollama nur an localhost gebunden

*Im vollständigen STREETS-Kurs zeigt dir Modul T (Technical Moats), wie du proprietäre Modellkonfigurationen, feinabgestimmte Pipelines und benutzerdefinierte Toolchains baust, die Wettbewerber nicht einfach replizieren können. Modul R (Revenue Engines) gibt dir die exakten Dienste, die du auf diesem Stack aufbauen sollst.*

---

## Lektion 3: Der Privatsphäre-Vorteil

*"Dein privates Setup IST ein Wettbewerbsvorteil — nicht nur eine Präferenz."*

### Privatsphäre ist ein Produktmerkmal, keine Einschränkung

Die meisten Entwickler richten lokale Infrastruktur ein, weil sie persönlich Privatsphäre schätzen oder weil sie gerne basteln. Das ist in Ordnung. Aber du lässt Geld auf dem Tisch liegen, wenn du nicht erkennst, dass **Privatsphäre eines der am besten vermarktbaren Merkmale in der Tech-Branche ist.**

Hier ist der Grund: Jedes Mal, wenn ein Unternehmen Daten an die OpenAI-API sendet, passieren diese Daten einen Drittanbieter. Für viele Unternehmen — besonders im Gesundheitswesen, Finanzwesen, Rechtswesen, bei Behörden und EU-basierten Unternehmen — ist das ein echtes Problem. Kein theoretisches. Ein "Wir können dieses Tool nicht nutzen, weil die Compliance-Abteilung Nein gesagt hat"-Problem.

Du, der Modelle lokal auf seinem Rechner ausführt, hast dieses Problem nicht.

### Der regulatorische Rückenwind

Das regulatorische Umfeld bewegt sich in deine Richtung. Schnell.

{? if regional.country == "US" ?}
> **US-basiert:** Die unten aufgeführten Vorschriften, die für dich am wichtigsten sind, sind HIPAA, SOC 2, ITAR und bundesstaatliche Datenschutzgesetze (Kalifornien CCPA usw.). EU-Vorschriften sind trotzdem relevant — sie betreffen deine Fähigkeit, europäische Kunden zu bedienen, was ein lukrativer Markt ist.
{? elif regional.country == "GB" ?}
> **UK-basiert:** Nach dem Brexit hat das UK sein eigenes Datenschutzrahmenwerk (UK GDPR + Data Protection Act 2018). Dein Vorteil lokaler Verarbeitung ist besonders stark für UK-Finanzdienstleistungen und NHS-nahe Arbeit.
{? elif regional.country == "DE" ?}
> **Deutschland-basiert:** Du bist in einer der strengsten Datenschutzumgebungen der Welt. Das ist ein *Vorteil* — deutsche Kunden verstehen bereits, warum lokale Verarbeitung wichtig ist, und sie werden dafür zahlen.
{? elif regional.country == "AU" ?}
> **Australien-basiert:** Der Privacy Act 1988 und die Australian Privacy Principles (APPs) regeln deine Pflichten. Lokale Verarbeitung ist ein starkes Verkaufsargument für Behörden- und Gesundheitskunden unter dem My Health Records Act.
{? endif ?}

**EU AI Act (in Kraft von 2024-2026):**
- Hochrisiko-KI-Systeme benötigen dokumentierte Datenverarbeitungs-Pipelines
- Unternehmen müssen nachweisen, wohin Daten fließen und wer sie verarbeitet
- Lokale Verarbeitung vereinfacht die Compliance drastisch
- EU-Unternehmen suchen aktiv nach KI-Dienstleistern, die EU-Datenspeicherung garantieren können

**DSGVO (bereits in Kraft):**
- "Datenverarbeitung" umfasst das Senden von Text an eine LLM-API
- Unternehmen brauchen Auftragsverarbeitungsverträge mit jedem Drittanbieter
- Lokale Verarbeitung eliminiert den Drittanbieter vollständig
- Das ist ein echtes Verkaufsargument: "Deine Daten verlassen niemals deine Infrastruktur. Es gibt keinen Drittanbieter-AVV zu verhandeln."

**Branchenspezifische Vorschriften:**
- **HIPAA (US-Gesundheitswesen):** Patientendaten können nicht an Consumer-KI-APIs gesendet werden, ohne einen BAA (Business Associate Agreement). Die meisten KI-Anbieter bieten keine BAAs für API-Zugriff an. Lokale Verarbeitung umgeht das vollständig.
- **SOC 2 (Enterprise):** Unternehmen, die SOC-2-Audits durchlaufen, müssen jeden Datenverarbeiter dokumentieren. Weniger Verarbeiter = einfachere Audits.
- **ITAR (US-Verteidigung):** Kontrollierte technische Daten dürfen die US-Jurisdiktion nicht verlassen. Cloud-KI-Anbieter mit internationaler Infrastruktur sind problematisch.
- **PCI DSS (Finanzen):** Die Verarbeitung von Karteninhaberdaten hat strenge Anforderungen daran, wohin Daten übertragen werden.

### Wie du Privatsphäre in Verkaufsgesprächen positionierst

Du musst kein Compliance-Experte sein. Du musst drei Phrasen verstehen und wissen, wann du sie einsetzt:

**Phrase 1: "Deine Daten verlassen niemals deine Infrastruktur."**
Verwenden wenn: Du mit einem datenschutzbewussten Interessenten sprichst. Das ist der universelle Aufhänger.

**Phrase 2: "Kein Auftragsverarbeitungsvertrag mit Drittanbietern erforderlich."**
Verwenden wenn: Du mit europäischen Unternehmen oder jedem Unternehmen mit Rechts-/Compliance-Team sprichst. Das spart ihnen Wochen an rechtlicher Prüfung.

**Phrase 3: "Vollständige Audit-Nachverfolgung, Single-Tenant-Verarbeitung."**
Verwenden wenn: Du mit Enterprise- oder regulierten Branchen sprichst. Sie müssen ihre KI-Pipeline gegenüber Auditoren nachweisen.

**Beispiel-Positionierung (für deine Dienstleistungsseite oder Angebote):**

> "Im Gegensatz zu Cloud-basierten KI-Diensten verarbeitet [Dein Dienst] alle Daten lokal auf dedizierter Hardware. Deine Dokumente, Code und Daten verlassen niemals die Verarbeitungsumgebung. Es gibt keine Drittanbieter-APIs in der Pipeline, keine Datenfreigabevereinbarungen zu verhandeln und vollständige Audit-Protokollierung jeder Operation. Das macht [Deinen Dienst] geeignet für Organisationen mit strengen Datenverarbeitungsanforderungen, einschließlich DSGVO, HIPAA und SOC-2-Compliance-Umgebungen."

Dieser Absatz auf einer Landingpage wird genau die Kunden anziehen, die Premium-Preise zahlen.

### Die Premium-Preisrechtfertigung

Hier ist der Business Case in harten Zahlen:

**Standard-KI-Verarbeitungsdienst (mit Cloud-APIs):**
- Die Daten des Kunden gehen an OpenAI/Anthropic/Google
- Du konkurrierst mit jedem Entwickler, der eine API aufrufen kann
- Marktpreis: $0,01-0,05 pro verarbeitetem Dokument
- Du verkaufst im Wesentlichen API-Zugang mit Aufschlag weiter

**Privatsphäre-zuerst-KI-Verarbeitungsdienst (dein lokaler Stack):**
- Die Daten des Kunden bleiben auf deinem Rechner
- Du konkurrierst mit einem viel kleineren Pool an Anbietern
- Marktpreis: $0,10-0,50 pro verarbeitetem Dokument (5-10x Premium)
- Du verkaufst Infrastruktur + Expertise + Compliance

Die Privatsphäre-Prämie ist real: **5x bis 10x** gegenüber Commodity-Cloud-basierten Diensten für die gleiche zugrundeliegende Aufgabe. Und die Kunden, die sie zahlen, sind loyaler, weniger preissensibel und haben größere Budgets.

{@ insight competitive_position @}

### Isolierte Arbeitsbereiche einrichten

Wenn du einen Hauptjob hast (die meisten von euch haben einen), brauchst du eine saubere Trennung zwischen Arbeitgeber-Arbeit und Einkommensarbeit. Das ist nicht nur rechtlicher Schutz — es ist operative Hygiene.

{? if computed.os_family == "windows" ?}
> **Windows-Tipp:** Erstelle ein separates Windows-Benutzerkonto für Einkommensarbeit (Einstellungen > Konten > Familie & andere Benutzer > Jemand anderen hinzufügen). Das gibt dir eine vollständig isolierte Umgebung — separate Browser-Profile, separate Dateipfade, separate Umgebungsvariablen. Wechsle zwischen Konten mit Win+L.
{? endif ?}

**Option 1: Separate Benutzerkonten (empfohlen)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**Option 2: Containerisierte Arbeitsbereiche**

```bash
# Docker-based isolation
# Create a dedicated workspace container

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
    # Your employer's VPN, tools, etc. are NOT in this container
```

**Option 3: Separater physischer Rechner (am sichersten)**

Wenn du es ernst meinst und dein Einkommen es rechtfertigt, eliminiert ein dedizierter Rechner alle Fragen. Ein gebrauchter Dell OptiPlex mit einer RTX 3060 kostet $400-600 und amortisiert sich im ersten Monat der Kundenarbeit.

**Minimale Trennungs-Checkliste:**
- [ ] Einkommensprojekte in einem separaten Verzeichnis (nie mit Arbeitgeber-Repos gemischt)
- [ ] Separate API-Keys für Einkommensarbeit (nie vom Arbeitgeber bereitgestellte Keys verwenden)
- [ ] Separates Browser-Profil für einkommensbezogene Konten
- [ ] Einkommensarbeit nie auf Arbeitgeber-Hardware
- [ ] Einkommensarbeit nie im Arbeitgeber-Netzwerk (nutze dein persönliches Internet oder ein VPN)
- [ ] Separates GitHub/GitLab-Konto für Einkommensprojekte (optional, aber sauber)

> **Häufiger Fehler:** Den OpenAI-API-Key deines Arbeitgebers "nur zum Testen" deines Nebenprojekts verwenden. Das erstellt eine Papierspur, die das Abrechnungs-Dashboard deines Arbeitgebers sehen kann, und es verwischt die IP-Grenzen. Hol dir deine eigenen Keys. Sie sind günstig.

### Lektion 3 Checkpoint

Du solltest jetzt verstehen:
- [ ] Warum Privatsphäre ein vermarktbares Produktmerkmal ist, nicht nur eine persönliche Präferenz
- [ ] Welche Vorschriften Nachfrage nach lokaler KI-Verarbeitung schaffen
- [ ] Drei Phrasen für Verkaufsgespräche über Privatsphäre
- [ ] Wie Privatsphäre-zuerst-Dienste 5-10x Premium-Preise erzielen
- [ ] Wie du Einkommensarbeit von Arbeitgeber-Arbeit trennst

*Im vollständigen STREETS-Kurs zeigt dir Modul E (Evolving Edge), wie du regulatorische Änderungen verfolgst und dich vor neuen Compliance-Anforderungen positionierst, bevor deine Wettbewerber überhaupt wissen, dass sie existieren.*

---

## Lektion 4: Das rechtliche Minimum

*"Fünfzehn Minuten rechtliches Setup jetzt verhindern Monate an Problemen später."*

### Dies ist keine Rechtsberatung

Ich bin Entwickler, kein Anwalt. Was folgt, ist eine praktische Checkliste, die die meisten Entwickler in den meisten Situationen abarbeiten sollten. Wenn deine Situation komplex ist (Beteiligung an deinem Arbeitgeber, Wettbewerbsverbot mit spezifischen Bedingungen usw.), investiere $200 in eine 30-minütige Beratung mit einem Arbeitsrechtsanwalt. Das ist der beste ROI, den du bekommen wirst.

### Schritt 1: Lies deinen Arbeitsvertrag

Finde deinen Arbeitsvertrag oder dein Angebotsschreiben. Suche nach diesen Abschnitten:

**Klausel zur Übertragung geistigen Eigentums** — Achte auf Formulierungen wie:
- "Alle Erfindungen, Entwicklungen und Arbeitsergebnisse..."
- "...die während der Beschäftigungsdauer erstellt wurden..."
- "...die mit dem Geschäft oder dem voraussichtlichen Geschäft des Unternehmens zusammenhängen..."

**Schlüsselphrasen, die dich einschränken:**
- "Alle während der Beschäftigung erstellten Arbeitsergebnisse gehören dem Unternehmen" (breit — potenziell problematisch)
- "Arbeitsergebnisse, die mit Unternehmensressourcen erstellt wurden" (enger — normalerweise in Ordnung, wenn du deine eigene Ausrüstung verwendest)
- "Im Zusammenhang mit dem aktuellen oder voraussichtlichen Geschäft des Unternehmens" (hängt davon ab, was dein Arbeitgeber macht)

**Schlüsselphrasen, die dich befreien:**
- "Ausgenommen Arbeiten, die ausschließlich in der Freizeit des Mitarbeiters mit eigenen Ressourcen und ohne Bezug zum Unternehmensgeschäft erstellt wurden" (das ist deine Ausnahmeregelung — viele US-Bundesstaaten verlangen dies)
- Einige Bundesstaaten (Kalifornien, Washington, Minnesota, Illinois und andere) haben Gesetze, die die IP-Ansprüche des Arbeitgebers auf persönliche Projekte begrenzen, unabhängig davon, was der Vertrag sagt.

### Der 3-Fragen-Test

Stelle dir für jedes Einkommensprojekt diese Fragen:

1. **Zeit:** Machst du diese Arbeit in deiner Freizeit? (Nicht während der Arbeitszeit, nicht während der Bereitschaftsdienste)
2. **Ausrüstung:** Verwendest du deine eigene Hardware, dein eigenes Internet, deine eigenen API-Keys? (Nicht den Arbeitgeber-Laptop, nicht das Arbeitgeber-VPN, nicht die Cloud-Konten des Arbeitgebers)
3. **Thema:** Steht das in keinem Zusammenhang mit dem Geschäft deines Arbeitgebers? (Wenn du bei einem KI-Unternehmen im Gesundheitswesen arbeitest und KI-Dienste fürs Gesundheitswesen verkaufen willst... ist das ein Problem. Wenn du bei einem KI-Unternehmen im Gesundheitswesen arbeitest und Dokumentenverarbeitung für Immobilienmakler verkaufen willst... ist das in Ordnung.)

Wenn alle drei Antworten sauber sind, bist du fast sicher in Ordnung. Wenn eine Antwort unklar ist, schaffe Klarheit, bevor du weitermachst.

> **Klartext:** Die überwältigende Mehrheit der Entwickler, die Nebenprojekte machen, hat nie ein Problem. Arbeitgeber kümmern sich um den Schutz von Wettbewerbsvorteilen, nicht darum, dich daran zu hindern, mit unverwandten Projekten zusätzliches Geld zu verdienen. Aber "fast sicher in Ordnung" ist nicht "definitiv in Ordnung." Wenn dein Vertrag ungewöhnlich breit ist, führe ein Gespräch mit deinem Vorgesetzten oder der Personalabteilung — oder konsultiere einen Anwalt. Die Konsequenz, es nicht zu prüfen, ist viel schlimmer als die leichte Unbehaglichkeit, nachzufragen.

### Schritt 2: Wähle eine Rechtsform

Du brauchst eine juristische Person, um dein Privatvermögen von deinen Geschäftstätigkeiten zu trennen und die Tür für Geschäftsbanking, Zahlungsabwicklung und Steuervorteile zu öffnen.

{? if regional.country ?}
> **Dein Standort: {= regional.country | fallback("Unknown") =}.** Die empfohlene Rechtsform für deine Region ist eine **{= regional.business_entity_type | fallback("LLC or equivalent") =}**, mit typischen Gründungskosten von {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Scrolle zum Abschnitt deines Landes unten, oder lies alle Abschnitte, um zu verstehen, wie Kunden in anderen Regionen operieren.
{? endif ?}

{? if regional.country == "US" ?}
#### Vereinigte Staaten (deine Region)
{? else ?}
#### Vereinigte Staaten
{? endif ?}

| Struktur | Kosten | Schutz | Am besten für |
|-----------|------|------------|----------|
| **Sole Proprietorship** (Standard) | $0 | Keiner (persönliche Haftung) | Erste Versuche. Die ersten $1K. |
| **Single-Member LLC** | $50-500 (variiert nach Bundesstaat) | Schutz des Privatvermögens | Aktive Einkommensarbeit. Die meisten Entwickler sollten hier beginnen. |
| **S-Corp-Wahl** (auf einer LLC) | LLC-Kosten + $0 für die Wahl | Wie LLC + Lohnsteuervorteile | Wenn du konstant $40K+/Jahr damit verdienst |

**Empfohlen für US-Entwickler:** Single-Member LLC in deinem Wohnsitz-Bundesstaat.

**Günstigste Bundesstaaten zur Gründung:** Wyoming ($100, keine staatliche Einkommensteuer), New Mexico ($50), Montana ($70). Aber die Gründung in deinem Heimatstaat ist normalerweise am einfachsten, es sei denn, du hast einen bestimmten Grund dagegen.

**Wie du anmeldest:**
1. Gehe auf die Website des Secretary of State deines Bundesstaates
2. Suche "form LLC" oder "business entity filing"
3. Reiche die Articles of Organization ein (10-Minuten-Formular)
4. Besorge dir eine EIN vom IRS (kostenlos, dauert 5 Minuten auf irs.gov)

{? if regional.country == "GB" ?}
#### Vereinigtes Königreich (deine Region)
{? else ?}
#### Vereinigtes Königreich
{? endif ?}

| Struktur | Kosten | Schutz | Am besten für |
|-----------|------|------------|----------|
| **Sole Trader** | Kostenlos (Registrierung bei HMRC) | Keiner | Erstes Einkommen. Testen. |
| **Limited Company (Ltd)** | ~$15 über Companies House | Schutz des Privatvermögens | Jede ernsthafte Einkommensarbeit. |

**Empfohlen:** Ltd-Company über Companies House. Das dauert etwa 20 Minuten und kostet 12 GBP.

#### Europäische Union

Variiert erheblich nach Land, aber das allgemeine Muster:

- **Deutschland:** Einzelunternehmer (Einzelunternehmen) zum Start, GmbH für ernsthafte Arbeit (aber GmbH erfordert 25.000 EUR Stammkapital — erwäge eine UG für 1 EUR)
- **Niederlande:** Eenmanszaak (Einzelunternehmen, kostenlose Registrierung) oder BV (vergleichbar mit Ltd)
- **Frankreich:** Micro-entrepreneur (vereinfacht, empfohlen zum Start)
- **Estland:** e-Residency + OUE (beliebt für Nicht-Ansässige, vollständig online)

{? if regional.country == "AU" ?}
#### Australien (deine Region)
{? else ?}
#### Australien
{? endif ?}

| Struktur | Kosten | Schutz | Am besten für |
|-----------|------|------------|----------|
| **Sole Trader** | Kostenlose ABN | Keiner | Zum Start |
| **Pty Ltd** | ~800-1200 AUD über ASIC | Schutz des Privatvermögens | Ernsthaftes Einkommen |

**Empfohlen:** Starte mit einer Sole-Trader-ABN (kostenlos, sofort), wechsle zu Pty Ltd, wenn du konstant verdienst.

### Schritt 3: Zahlungsabwicklung (15-Minuten-Setup)

Du brauchst eine Möglichkeit, bezahlt zu werden. Richte das jetzt ein, nicht wenn dein erster Kunde wartet.

{? if regional.payment_processors ?}
> **Empfohlen für {= regional.country | fallback("your region") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (empfohlen für die meisten Entwickler):**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

Zeitaufwand: ~15 Minuten. Du kannst sofort Zahlungen annehmen (Stripe hält Gelder bei neuen Konten 7 Tage zurück).

**Lemon Squeezy (empfohlen für digitale Produkte):**

Wenn du digitale Produkte verkaufst (Templates, Tools, Kurse, SaaS), agiert Lemon Squeezy als dein Merchant of Record. Das bedeutet:
- Sie übernehmen Umsatzsteuer, Mehrwertsteuer und GST weltweit für dich
- Du musst dich nicht für die Umsatzsteuer in der EU registrieren
- Sie übernehmen Rückerstattungen und Streitigkeiten

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (für internationale Entwickler oder solche, die eine US-Entität wollen):**

Wenn du außerhalb der USA bist, aber an US-Kunden mit einer US-Entität verkaufen willst:
- $500 einmalige Gebühr
- Erstellt eine Delaware LLC für dich
- Richtet ein US-Bankkonto ein (über Mercury oder Stripe)
- Bietet Registered-Agent-Service
- Dauert etwa 1-2 Wochen

### Schritt 4: Datenschutzerklärung und Nutzungsbedingungen

Wenn du irgendeinen Dienst oder ein Produkt online verkaufst, brauchst du diese. Zahle keinen Anwalt für Standard-Vorlagen.

**Kostenlose, seriöse Quellen für Vorlagen:**
- **Termly.io** — Kostenloser Generator für Datenschutzerklärung und Nutzungsbedingungen. Fragen beantworten, Dokumente erhalten.
- **Avodocs.com** — Open-Source-Rechtsdokumente für Startups. Kostenlos.
- **GitHubs choosealicense.com** — Speziell für Open-Source-Projektlizenzen.
- **Basecamps Open-Source-Richtlinien** — Suche nach "Basecamp open source policies" — gute, verständliche Vorlagen.

**Was deine Datenschutzerklärung abdecken muss (wenn du Kundendaten verarbeitest):**
- Welche Daten du erhebst
- Wie du sie verarbeitest (lokal — das ist dein Vorteil)
- Wie lange du sie aufbewahrst
- Wie Kunden die Löschung beantragen können
- Ob Dritte auf die Daten zugreifen (idealerweise: keine)

**Zeitaufwand:** 30 Minuten mit einem Template-Generator. Erledigt.

### Schritt 5: Separates Geschäftskonto

Lass Geschäftseinnahmen nicht über dein privates Girokonto laufen. Die Gründe:

1. **Steuerliche Klarheit:** Wenn die Steuerzeit kommt, musst du genau wissen, was Geschäftseinkommen war und was nicht.
2. **Rechtlicher Schutz:** Wenn du eine LLC hast, kann die Vermischung von privaten und geschäftlichen Geldern den "Corporate Veil durchstoßen" — das bedeutet, ein Gericht kann den Haftungsschutz deiner LLC ignorieren.
3. **Professionalität:** Rechnungen von "Johns Consulting LLC" auf ein dediziertes Geschäftskonto sehen seriös aus. Zahlungen an dein persönliches Venmo nicht.

**Kostenloses oder günstiges Geschäftsbanking:**
{? if regional.country == "US" ?}
- **Mercury** (empfohlen für dich) — Kostenlos, für Startups konzipiert. Exzellente API, wenn du später die Buchhaltung automatisieren willst.
- **Relay** — Kostenlos, gut für die Trennung von Einkommensströmen in Unterkonten.
{? elif regional.country == "GB" ?}
- **Starling Bank** (empfohlen für dich) — Kostenloses Geschäftskonto, sofortige Einrichtung.
- **Wise Business** — Günstig, multi-währungsfähig. Ideal, wenn du internationale Kunden bedienst.
{? else ?}
- **Mercury** (US) — Kostenlos, für Startups konzipiert. Exzellente API, wenn du später die Buchhaltung automatisieren willst.
- **Relay** (US) — Kostenlos, gut für die Trennung von Einkommensströmen in Unterkonten.
- **Starling Bank** (UK) — Kostenloses Geschäftskonto.
{? endif ?}
- **Wise Business** (International) — Günstig, multi-währungsfähig. Ideal für den Empfang von Zahlungen in USD, EUR, GBP usw.
- **Qonto** (EU) — Sauberes Geschäftsbanking für europäische Unternehmen.

Eröffne das Konto jetzt. Es dauert 10-15 Minuten online und 1-3 Tage für die Verifizierung.

### Schritt 6: Steuergrundlagen für Entwickler-Nebeneinkommen

{? if regional.tax_note ?}
> **Steuerhinweis für {= regional.country | fallback("your region") =}:** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **Klartext:** Steuern sind das, was die meisten Entwickler bis April ignorieren und dann in Panik geraten. 30 Minuten jetzt zu investieren spart dir echtes Geld und Stress.

**Vereinigte Staaten:**
- Nebeneinkommen über $400/Jahr erfordert Self-Employment-Tax (~15,3% für Social Security + Medicare)
- Plus dein regulärer Einkommensteuersatz auf den Nettogewinn
- **Vierteljährliche geschätzte Steuern:** Wenn du mehr als $1.000 an Steuern schulden wirst, erwartet das IRS vierteljährliche Zahlungen (15. April, 15. Juni, 15. September, 15. Januar). Unterzahlung löst Strafen aus.
- Lege **25-30%** des Nettoeinkommens für Steuern zurück. Überweise es sofort auf ein separates Sparkonto.

**Häufige Absetzungen für Entwickler-Nebeneinkommen:**
- API-Kosten (OpenAI, Anthropic usw.) — 100% absetzbar
- Hardwarekäufe für geschäftliche Nutzung — abschreibbar oder Section-179-Abzug
- Stromkosten, die dem geschäftlichen Gebrauch zuzuordnen sind
- Software-Abonnements für Einkommensarbeit
- Homeoffice-Abzug (vereinfacht: $5/sq ft, bis zu 300 sq ft = $1.500)
- Internet (Anteil geschäftlicher Nutzung)
- Domainnamen, Hosting, E-Mail-Dienste
- Berufliche Weiterbildung (Kurse, Bücher) im Zusammenhang mit deiner Einkommensarbeit

**Vereinigtes Königreich:**
- Melde über die Self-Assessment-Steuererklärung
- Handelseinkommen unter 1.000 GBP: steuerfrei (Trading Allowance)
- Darüber: zahle Einkommensteuer + Class-4-NICs auf Gewinne
- Zahlungstermine: 31. Januar und 31. Juli

**Tracke alles von Tag eins an.** Nutze notfalls eine einfache Tabelle:

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **Häufiger Fehler:** "Ich kümmere mich später um Steuern." Später ist Q4, du schuldest $3.000 an geschätzten Steuern plus Strafen, und du hast das Geld ausgegeben. Automatisiere: Jedes Mal, wenn Einkommen auf deinem Geschäftskonto eingeht, überweise sofort 30% auf ein Steuer-Sparkonto.

### Lektion 4 Checkpoint

Du solltest jetzt haben (oder einen Plan dafür):
- [ ] IP-Klausel deines Arbeitsvertrags gelesen
- [ ] Den 3-Fragen-Test für deine geplante Einkommensarbeit bestanden
- [ ] Eine Rechtsform gewählt (oder entschieden, als Einzelunternehmer zu starten)
- [ ] Zahlungsabwicklung eingerichtet (Stripe oder Lemon Squeezy)
- [ ] Datenschutzerklärung und Nutzungsbedingungen aus einem Template-Generator
- [ ] Separates Geschäftsbankkonto (oder Antrag eingereicht)
- [ ] Steuerstrategie: 30% Rücklage + vierteljährlicher Zahlungsplan

*Im vollständigen STREETS-Kurs enthält Modul E (Execution Playbook) Finanzmodellierungs-Templates, die automatisch deine Steuerverpflichtungen, Projektrentabilität und Break-even-Punkte für jede Revenue Engine berechnen.*

---

## Lektion 5: Das {= regional.currency_symbol | fallback("$") =}200/Monats-Budget

*"Dein Unternehmen hat eine Burn Rate. Kenne sie. Kontrolliere sie. Lass sie verdienen."*

### Warum {= regional.currency_symbol | fallback("$") =}200/Monat

Zweihundert {= regional.currency | fallback("dollars") =} pro Monat ist das minimal tragfähige Budget für einen Entwickler-Einkommensbetrieb. Es reicht, um echte Dienste zu betreiben, echte Kunden zu bedienen und echten Umsatz zu generieren. Es ist auch klein genug, dass du nicht alles auf eine Karte gesetzt hast, wenn nichts funktioniert.

Das Ziel ist einfach: **Verwandle {= regional.currency_symbol | fallback("$") =}200/Monat in {= regional.currency_symbol | fallback("$") =}600+/Monat innerhalb von 90 Tagen.** Wenn du das schaffst, hast du ein Geschäft. Wenn nicht, änderst du die Strategie — nicht das Budget.

### Die Budget-Aufschlüsselung

#### Stufe 1: API-Credits — $50-100/Monat

Das ist deine Produktions-Rechenleistung für kundenorientierte Qualität.

**Empfohlene Startverteilung:**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**Wie du API-Ausgaben verwaltest:**

```python
# Simple API budget tracker — run daily via cron
# Save as: check_api_spend.py

import requests
import json
from datetime import datetime

# Check Anthropic usage
# (Anthropic provides usage in the dashboard; here's how to track locally)

MONTHLY_BUDGET = {
    "anthropic": 40.00,
    "openai": 20.00,
    "deepseek": 10.00,
}

# Track locally by logging every API call cost
USAGE_LOG = "api_usage.jsonl"

def get_monthly_spend(provider: str) -> float:
    """Calculate current month's spend for a provider."""
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
    """Log an API call for budget tracking."""
    # Cost per 1M tokens (update these as pricing changes)
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

    # Budget warning
    monthly_spend = get_monthly_spend(provider)
    budget = MONTHLY_BUDGET.get(provider, 0)
    if monthly_spend > budget * 0.8:
        print(f"WARNING: {provider} spend at {monthly_spend:.2f}/{budget:.2f} "
              f"({monthly_spend/budget*100:.0f}%)")

    return cost
```

**Die Hybrid-Ausgabestrategie:**
- Nutze lokale LLMs für 80% der Verarbeitung (Klassifizierung, Extraktion, Zusammenfassung, Entwürfe)
- Nutze API-Aufrufe für 20% der Verarbeitung (finale Qualitätsprüfung, komplexes Reasoning, kundenorientierte Ausgaben)
- Deine effektiven Kosten pro Aufgabe sinken dramatisch im Vergleich zu reiner API-Nutzung

{? if computed.monthly_electricity_estimate ?}
> **Deine geschätzten Stromkosten:** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/Monat für 24/7-Betrieb bei {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh. Das ist bereits in deine effektiven Betriebskosten eingerechnet.
{? endif ?}

#### Stufe 2: Infrastruktur — {= regional.currency_symbol | fallback("$") =}30-50/Monat

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**Brauchst du einen VPS?**

Wenn dein Einkommensmodell ist:
- **Digitale Produkte verkaufen:** Nein. Hoste auf Vercel/Netlify kostenlos. Nutze Lemon Squeezy für die Auslieferung.
- **Asynchrone Verarbeitung für Kunden:** Vielleicht. Du kannst Jobs auf deinem lokalen Rechner ausführen und Ergebnisse liefern. Ein VPS erhöht die Zuverlässigkeit.
- **Einen API-Dienst anbieten:** Ja, wahrscheinlich. Ein $5-10 VPS fungiert als leichtgewichtiges API-Gateway, auch wenn die schwere Verarbeitung auf deinem lokalen Rechner stattfindet.
- **SaaS verkaufen:** Ja. Aber starte mit der günstigsten Stufe und skaliere hoch.

**Empfohlene Starter-Infrastruktur:**

```
Local rig — primary compute, LLM inference, heavy processing
   |
   +-- SSH tunnel or WireGuard VPN
   |
$5 VPS (Hetzner/DigitalOcean) — API gateway, webhook receiver, static hosting
   |
   +-- Cloudflare (free) — DNS, CDN, DDoS protection
   |
Vercel/Netlify (free) — marketing site, landing pages, docs
```

Gesamte Infrastrukturkosten: $5-20/Monat. Der Rest sind kostenlose Tarife.

#### Stufe 3: Tools — {= regional.currency_symbol | fallback("$") =}20-30/Monat

```
Analytics:              $0/month    — Plausible Cloud ($9) or self-hosted,
                                      or Vercel Analytics (free tier)
                                      or just Cloudflare analytics (free)
Email marketing:        $0/month    — Buttondown (free up to 100 subs),
                                      Resend ($0 for 3K emails/month)
Monitoring:             $0/month    — UptimeRobot (free, 50 monitors),
                                      Better Stack (free tier)
Design:                 $0/month    — Figma (free), Canva (free tier)
Accounting:             $0/month    — Wave (free), or a spreadsheet
                                      Hledger (free, plaintext accounting)
```

> **Klartext:** Du kannst deinen gesamten Tool-Stack am Anfang auf kostenlosen Tarifen betreiben. Die $20-30, die hier eingeplant sind, sind für den Fall, dass du über kostenlose Tarife hinauswächst oder ein bestimmtes Premium-Feature willst. Gib es nicht aus, nur weil es im Budget steht. Nicht ausgegebenes Budget ist Gewinn.

#### Stufe 4: Reserve — {= regional.currency_symbol | fallback("$") =}0-30/Monat

Das ist dein "Unvorhergesehenes"-Fonds:
- Ein API-Kostenspike durch einen unerwartet großen Batch-Job
- Ein Tool, das du für ein bestimmtes Kundenprojekt brauchst
- Notfall-Domain-Kauf, wenn du den perfekten Namen findest
- Ein Einmalkauf (Theme, Template, Icon-Set)

Wenn du die Reserve nicht nutzt, sammelt sie sich an. Nach 3 Monaten ungenutzter Reserve erwäge eine Umverteilung auf API-Credits oder Infrastruktur.

### Die ROI-Berechnung

Das ist die einzige Zahl, die zählt:

```
Monthly Revenue - Monthly Costs = Net Profit
Net Profit / Monthly Costs = ROI Multiple

Example:
$600 revenue - $200 costs = $400 profit
$400 / $200 = 2x ROI

The target: 3x ROI ($600+ revenue on $200 spend)
The minimum: 1x ROI ($200 revenue = break even)
Below 1x: Change strategy or reduce costs
```

{@ insight cost_projection @}

**Wann das Budget erhöhen:**

Erhöhe dein Budget NUR wenn:
1. Du konstant bei 2x+ ROI für 2+ Monate bist
2. Mehr Ausgaben direkt mehr Umsatz bedeuten würden (z.B. mehr API-Credits = mehr Kundenkapazität)
3. Die Erhöhung an einen spezifischen, getesteten Einkommensstrom gebunden ist

**Wann das Budget NICHT erhöhen:**
- "Ich glaube, dieses neue Tool wird helfen" (teste zuerst kostenlose Alternativen)
- "Alle sagen, man muss Geld ausgeben, um Geld zu verdienen" (nicht in diesem Stadium)
- "Ein größerer VPS wird meinen Dienst schneller machen" (ist Geschwindigkeit tatsächlich der Engpass?)
- Du hast noch nicht 1x ROI erreicht (verbessere den Umsatz, nicht die Ausgaben)

**Die Skalierungsleiter:**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **Häufiger Fehler:** Die {= regional.currency_symbol | fallback("$") =}200 als "Investition" behandeln, die nicht sofort Geld zurückbringen muss. Nein. Das ist ein Experiment mit einer 90-Tage-Frist. Wenn {= regional.currency_symbol | fallback("$") =}200/Monat nicht innerhalb von 90 Tagen {= regional.currency_symbol | fallback("$") =}200/Monat an Umsatz generieren, muss sich etwas an der Strategie ändern. Das Geld, der Markt, das Angebot — irgendetwas funktioniert nicht. Sei ehrlich zu dir selbst.

### Lektion 5 Checkpoint

Du solltest jetzt haben:
- [ ] Ein monatliches Budget von ~$200, aufgeteilt auf vier Stufen
- [ ] API-Konten erstellt mit gesetzten Ausgabenlimits
- [ ] Infrastruktur-Entscheidungen getroffen (nur lokal vs. lokal + VPS)
- [ ] Einen Tool-Stack ausgewählt (hauptsächlich kostenlose Tarife zum Start)
- [ ] ROI-Ziele: 3x innerhalb von 90 Tagen
- [ ] Eine klare Regel: Budget nur nach bewiesenem ROI erhöhen

*Im vollständigen STREETS-Kurs enthält Modul E (Execution Playbook) ein Finanz-Dashboard-Template, das deine Ausgaben, deinen Umsatz und deinen ROI pro Revenue Engine in Echtzeit verfolgt — damit du immer weißt, welche Ströme profitabel sind und welche Anpassung brauchen.*

---

## Lektion 6: Dein Sovereign Stack Dokument

*"Jedes Unternehmen hat einen Plan. Das ist deiner — und er passt auf zwei Seiten."*

### Das Ergebnis

Das ist das Wichtigste, was du in Modul S erstellen wirst. Dein Sovereign Stack Dokument ist eine einzelne Referenz, die alles über deine einkommensgenerierende Infrastruktur festhält. Du wirst es im gesamten Rest des STREETS-Kurses referenzieren, es aktualisieren, wenn sich dein Setup entwickelt, und es nutzen, um klare Entscheidungen darüber zu treffen, was du bauen und was du überspringen sollst.

Erstelle eine neue Datei. Markdown, Google Doc, Notion-Seite, reiner Text — was auch immer du tatsächlich pflegen wirst. Nutze das Template unten und fülle jedes Feld mit den Zahlen und Entscheidungen aus den Lektionen 1-5 aus.

### Das Template

{? if computed.profile_completeness != "0" ?}
> **Vorsprung:** 4DA hat bereits einige deiner Hardware-Spezifikationen und Stack-Infos erkannt. Achte auf die vorausgefüllten Hinweise unten — sie sparen dir Zeit beim Ausfüllen des Templates.
{? endif ?}

Kopiere dieses gesamte Template und fülle es aus. Jedes Feld. Nichts überspringen.

```markdown
# Sovereign Stack Document
# [Your Name or Business Name]
# Created: [Date]
# Last Updated: [Date]

---

## 1. HARDWARE INVENTORY

### Primary Machine
- **Type:** [Desktop / Laptop / Mac / Server]
- **CPU:** [Model] — [X] cores, [X] threads
- **RAM:** [X] GB [DDR4/DDR5]
- **GPU:** [Model] — [X] GB VRAM (or "None — CPU inference only")
- **Storage:** [X] GB SSD free / [X] GB total
- **OS:** [Linux distro / macOS version / Windows version]

### Network
- **Download:** [X] Mbps
- **Upload:** [X] Mbps
- **Latency to cloud APIs:** [X] ms
- **ISP reliability:** [Stable / Occasional outages / Unreliable]

### Uptime Capability
- **Can run 24/7:** [Yes / No — reason]
- **UPS:** [Yes / No]
- **Remote access:** [SSH / RDP / Tailscale / None]

### Monthly Infrastructure Cost
- **Electricity (24/7 estimate):** $[X]/month
- **Internet:** $[X]/month (business portion)
- **Total fixed infrastructure cost:** $[X]/month

---

## 2. LLM STACK

### Local Models (via Ollama)
| Model | Size | Tokens/sec | Use Case |
|-------|------|-----------|----------|
| [e.g., llama3.1:8b] | [X]B | [X] tok/s | [e.g., Classification, extraction] |
| [e.g., mistral:7b] | [X]B | [X] tok/s | [e.g., Summarization, drafts] |
| [e.g., deepseek-coder] | [X]B | [X] tok/s | [e.g., Code generation] |

### API Models (for quality-critical output)
| Provider | Model | Monthly Budget | Use Case |
|----------|-------|---------------|----------|
| [e.g., Anthropic] | [Claude 3.5 Sonnet] | $[X] | [e.g., Customer-facing content] |
| [e.g., OpenAI] | [GPT-4o-mini] | $[X] | [e.g., Volume processing fallback] |

### Inference Strategy
- **Local handles:** [X]% of requests ([list tasks])
- **API handles:** [X]% of requests ([list tasks])
- **Estimated blended cost per 1M tokens:** $[X]

---

## 3. MONTHLY BUDGET

| Category | Allocation | Actual (update monthly) |
|----------|-----------|------------------------|
| API Credits | $[X] | $[  ] |
| Infrastructure (VPS, domain, email) | $[X] | $[  ] |
| Tools (analytics, email marketing) | $[X] | $[  ] |
| Reserve | $[X] | $[  ] |
| **Total** | **$[X]** | **$[  ]** |

### Revenue Target
- **Month 1-3:** $[X]/month (minimum: cover costs)
- **Month 4-6:** $[X]/month
- **Month 7-12:** $[X]/month

---

## 4. LEGAL STATUS

- **Employment status:** [Employed / Freelance / Between jobs]
- **IP clause reviewed:** [Yes / No / N/A]
- **IP clause risk level:** [Clean / Murky — needs review / Restrictive]
- **Business entity:** [LLC / Ltd / Sole Proprietor / None yet]
  - **State/Country:** [Where registered]
  - **EIN/Tax ID:** [Obtained / Pending / Not needed yet]
- **Payment processing:** [Stripe / Lemon Squeezy / Other] — [Active / Pending]
- **Business bank account:** [Open / Pending / Using personal (fix this)]
- **Privacy policy:** [Done / Not yet — URL: ___]
- **Terms of service:** [Done / Not yet — URL: ___]

---

## 5. TIME INVENTORY

- **Available hours per week for income projects:** [X] hours
  - **Weekday mornings:** [X] hours
  - **Weekday evenings:** [X] hours
  - **Weekends:** [X] hours
- **Time zone:** [Your timezone]
- **Best deep work blocks:** [e.g., "Saturday 6am-12pm, weekday evenings 8-10pm"]

### Time Allocation Plan
| Activity | Hours/week |
|----------|-----------|
| Building/coding | [X] |
| Marketing/sales | [X] |
| Client work/delivery | [X] |
| Learning/experimentation | [X] |
| Admin (invoicing, email, etc.) | [X] |

> Rule: Never allocate more than 70% of available time.
> Life happens. Burnout is real. Leave buffer.

---

## 6. SKILLS INVENTORY

### Primary Skills (things you could teach others)
1. [Skill] — [years of experience]
2. [Skill] — [years of experience]
3. [Skill] — [years of experience]

### Secondary Skills (competent but not expert)
1. [Skill]
2. [Skill]
3. [Skill]

### Exploring (learning now or want to learn)
1. [Skill]
2. [Skill]

### Unique Combinations
What makes YOUR skill combination unusual? (This becomes your moat in Module T)
- [e.g., "I know both Rust AND healthcare data standards — very few people have both"]
- [e.g., "I can build full-stack apps AND I understand supply chain logistics from a previous career"]
- [e.g., "I'm fluent in 3 languages AND I can code — I can serve non-English markets that most dev tools ignore"]

---

## 7. SOVEREIGN STACK SUMMARY

### What I Can Offer Today
(Based on hardware + skills + time, what could you sell THIS WEEK if someone asked?)
1. [e.g., "Local document processing — extract data from PDFs privately"]
2. [e.g., "Custom automation scripts for [specific domain]"]
3. [e.g., "Technical writing / documentation"]

### What I'm Building Toward
(Based on the full STREETS framework — fill this in as you progress through the course)
1. [Revenue Engine 1 — from Module R]
2. [Revenue Engine 2 — from Module R]
3. [Revenue Engine 3 — from Module R]

### Key Constraints
(Be honest — these aren't weaknesses, they're parameters)
- [e.g., "Only 10 hours/week available"]
- [e.g., "No GPU — CPU inference only, will rely on APIs for LLM tasks"]
- [e.g., "Employment contract is restrictive — need to stay in unrelated domains"]
- [e.g., "Non-US based — some payment/legal options are limited"]

---

*This document is a living reference. Update it monthly.*
*Next review date: [Date + 30 days]*
```

{? if dna.primary_stack ?}
> **Vorausfüllung aus deiner Developer DNA:**
> - **Primärer Stack:** {= dna.primary_stack | fallback("Not detected") =}
> - **Interessen:** {= dna.interests | fallback("Not detected") =}
> - **Identitätszusammenfassung:** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **Blinde Flecken beachten:** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **Vorausfüllung aus erkanntem Stack:** Deine Primärtechnologien sind {= stack.primary | fallback("not yet detected") =}. {? if stack.adjacent ?}Angrenzende Skills: {= stack.adjacent | fallback("none detected") =}.{? endif ?} Nutze diese, um das Skills-Inventar oben auszufüllen.
{? endif ?}

{@ insight t_shape @}

### Wie du dieses Dokument verwendest

1. **Vor dem Start eines neuen Projekts:** Prüfe deinen Sovereign Stack. Hast du die Hardware, Zeit, Skills und das Budget zur Umsetzung?
2. **Vor jedem Kauf:** Prüfe deine Budget-Verteilung. Ist dieser Kauf im Plan?
3. **Monatliche Überprüfung:** Aktualisiere die "Actual"-Spalte in deinem Budget. Aktualisiere die Umsatzzahlen. Passe die Verteilung basierend auf dem an, was funktioniert.
4. **Wenn jemand fragt, was du machst:** Dein Abschnitt "Was ich heute anbieten kann" ist dein sofortiger Pitch.
5. **Wenn du versucht bist, einer glänzenden neuen Idee nachzujagen:** Prüfe deine Einschränkungen. Passt das in deine Zeit, Skills und Hardware? Wenn nicht, füge es zu "Wohin ich arbeite" für später hinzu.

### Die Einstündige Übung

Stelle einen Timer auf 60 Minuten. Fülle jedes Feld des Templates aus. Überdenke es nicht. Recherchiere nicht ausführlich. Schreibe auf, was du jetzt weißt. Du kannst es später aktualisieren.

Die Felder, die du nicht ausfüllen kannst? Das sind deine Aufgaben für diese Woche:
- Leere Benchmark-Zahlen? Führe das Benchmark-Skript aus Lektion 2 aus.
- Keine Rechtsform? Starte den Anmeldeprozess aus Lektion 4.
- Keine Zahlungsabwicklung? Richte Stripe aus Lektion 4 ein.
- Leeres Skills-Inventar? Nimm dir 15 Minuten und liste alles auf, wofür du in den letzten 5 Jahren bezahlt wurdest.

> **Häufiger Fehler:** 3 Stunden damit verbringen, das Dokument "perfekt" zu machen, statt 1 Stunde, um es "fertig" zu machen. Das Sovereign Stack Dokument ist eine Arbeitsreferenz, kein Businessplan für Investoren. Niemand außer dir wird es sehen. Genauigkeit zählt. Formatierung nicht.

### Lektion 6 Checkpoint

Du solltest jetzt haben:
- [ ] Ein vollständiges Sovereign Stack Dokument, gespeichert an einem Ort, den du tatsächlich öffnen wirst
- [ ] Alle sechs Abschnitte mit echten Zahlen ausgefüllt (nicht erstrebenswerten)
- [ ] Eine klare Liste von Aufgaben für Lücken in deinem Setup
- [ ] Ein Datum für deine erste monatliche Überprüfung (30 Tage ab jetzt)

---

## Modul S: Abgeschlossen

{? if progress.completed("MODULE_S") ?}
> **Modul S abgeschlossen.** Du hast {= progress.completed_count | fallback("1") =} von {= progress.total_count | fallback("7") =} STREETS-Modulen abgeschlossen. {? if progress.completed_modules ?}Abgeschlossen: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### Was du in zwei Wochen aufgebaut hast

Schau dir an, was du jetzt hast, was du am Anfang nicht hattest:

1. **Ein Hardware-Inventar**, das auf einkommensgenerierende Fähigkeiten abgebildet ist — nicht nur Spezifikationen auf einem Aufkleber.
2. **Einen produktionsreifen lokalen LLM-Stack** mit Ollama, auf deiner tatsächlichen Hardware benchmarked, für echte Workloads konfiguriert.
3. **Einen Privatsphäre-Vorteil**, den du zu vermarkten weißt — mit spezifischer Sprache für spezifische Zielgruppen.
4. **Ein rechtliches und finanzielles Fundament** — Rechtsform (oder Plan), Zahlungsabwicklung, Bankkonto, Steuerstrategie.
5. **Ein kontrolliertes Budget** mit klaren ROI-Zielen und einer 90-Tage-Frist, um das Modell zu beweisen.
6. **Ein Sovereign Stack Dokument**, das all das oben in einer einzelnen Referenz festhält, die du für jede zukünftige Entscheidung verwenden wirst.

Das ist mehr, als die meisten Entwickler jemals einrichten. Ernsthaft. Die meisten Leute, die Nebeneinkommen verdienen wollen, springen direkt zu "baue etwas Cooles" und wundern sich dann, warum sie nicht bezahlt werden. Du hast jetzt die Infrastruktur, um bezahlt zu werden.

Aber Infrastruktur ohne Richtung ist nur ein teures Hobby. Du musst wissen, wohin du diesen Stack richten sollst.

{@ temporal market_timing @}

### Was als Nächstes kommt: Modul T — Technical Moats

Modul S hat dir das Fundament gegeben. Modul T beantwortet die entscheidende Frage: **Wie baust du etwas, das Wettbewerber nicht einfach kopieren können?**

Hier ist, was Modul T abdeckt:

- **Proprietäre Daten-Pipelines** — wie du Datensätze erstellst, auf die nur du Zugriff hast, legal und ethisch
- **Benutzerdefinierte Modellkonfigurationen** — Feinabstimmung und Prompt Engineering, das eine Ausgabequalität produziert, die andere mit Standardeinstellungen nicht erreichen können
- **Sich verstärkende Skill-Stacks** — warum "Python + Gesundheitswesen" "Python + JavaScript" für Einkommen schlägt, und wie du deine einzigartige Kombination identifizierst
- **Technische Eintrittsbarrieren** — Infrastruktur-Designs, die ein Wettbewerber Monate bräuchte, um zu replizieren
- **Das Moat-Audit** — ein Framework zur Bewertung, ob dein Projekt einen verteidigbaren Vorteil hat oder nur ein weiterer Commodity-Dienst ist

Der Unterschied zwischen einem Entwickler, der $500/Monat verdient, und einem, der $5.000/Monat verdient, ist selten Können. Es sind Moats. Dinge, die dein Angebot schwer replizierbar machen, selbst wenn jemand die gleiche Hardware und die gleichen Modelle hat.

### Die vollständige STREETS-Roadmap

| Modul | Titel | Fokus | Dauer |
|--------|-------|-------|----------|
| **S** | Sovereign Setup | Infrastruktur, Recht, Budget | Wochen 1-2 (abgeschlossen) |
| **T** | Technical Moats | Verteidigbare Vorteile, proprietäre Assets | Wochen 3-4 |
| **R** | Revenue Engines | Spezifische Monetarisierungs-Playbooks mit Code | Wochen 5-8 |
| **E** | Execution Playbook | Launch-Sequenzen, Preisgestaltung, erste Kunden | Wochen 9-10 |
| **E** | Evolving Edge | Vorne bleiben, Trend-Erkennung, Anpassung | Wochen 11-12 |
| **T** | Tactical Automation | Automatisierung des Betriebs für passives Einkommen | Wochen 13-14 |
| **S** | Stacking Streams | Mehrere Einkommensquellen, Portfolio-Strategie | Wochen 15-16 |

Modul R (Revenue Engines) ist da, wo das meiste Geld verdient wird. Aber ohne S und T baust du auf Sand.

---

**Bereit für das vollständige Playbook?**

Du hast das Fundament gesehen. Du hast es selbst gebaut. Jetzt hol dir das komplette System.

**Hol dir STREETS Core** — den vollständigen 16-Wochen-Kurs mit allen sieben Modulen, Revenue-Engine-Code-Templates, Finanz-Dashboards und der privaten Community von Entwicklern, die Einkommen zu ihren eigenen Bedingungen aufbauen.
