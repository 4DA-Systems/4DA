# Modulo E: Playbook di Esecuzione

**Corso STREETS per il Reddito da Sviluppatore — Modulo a Pagamento**
*Settimane 9-10 | 6 Lezioni | Deliverable: Il Tuo Primo Prodotto, Online e Pronto a Ricevere Pagamenti*

> "Da idea a deploy in 48 ore. Niente overthinking."

---

Hai l'infrastruttura (Modulo S). Hai il vantaggio competitivo (Modulo T). Hai i progetti per il motore di revenue (Modulo R). Adesso è il momento di spedire.

Questo modulo è quello che la maggior parte degli sviluppatori non raggiunge mai — non perché sia difficile, ma perché stanno ancora lucidando il codice, rifattorizzando l'architettura, aggiustando la palette di colori. Stanno facendo tutto tranne l'unica cosa che conta: mettere un prodotto davanti a un essere umano che può pagare.

Spedire è una competenza. Come ogni competenza, migliora con la pratica e peggiora con il ritardo. Più aspetti, più diventa difficile. Più spedisci, meno fa paura. Il tuo primo lancio sarà caotico. Questo è il punto.

Entro la fine di queste due settimane, avrai:

- Un'idea di prodotto validata testata contro segnali di domanda reali
- Un prodotto live, deployato e accessibile tramite un dominio reale
- Un sistema di pagamento che accetta denaro vero
- Almeno un lancio pubblico su una piattaforma dove il tuo pubblico target si riunisce
- Un sistema di metriche post-lancio per guidare le tue prossime mosse

Niente ipotesi. Niente "in teoria." Un prodotto reale, live su internet, capace di generare entrate.

{? if progress.completed("R") ?}
Hai completato il Modulo R — hai già dei design per motori di revenue pronti all'esecuzione. Questo modulo trasforma uno di quei design in un prodotto live.
{? else ?}
Se non hai ancora completato il Modulo R, puoi comunque usare questo modulo — ma avere un design per il motore di revenue già pronto renderà lo sprint di 48 ore significativamente più fluido.
{? endif ?}

{@ mirror execution_readiness @}

Costruiamolo.

---

## Lezione 1: Lo Sprint di 48 Ore

*"Da sabato mattina a domenica sera. Un prodotto. Zero scuse."*

### Perché 48 Ore

La Legge di Parkinson dice che il lavoro si espande fino a riempire il tempo disponibile. Datti 6 mesi per costruire un prodotto e ne passerai 5 a deliberare e 1 in una frenesia stressata. Datti 48 ore e prenderai decisioni, taglierai lo scope senza pietà e spedirai qualcosa di reale.

Il vincolo delle 48 ore non serve a costruire qualcosa di perfetto. Serve a costruire qualcosa che esiste. L'esistenza batte la perfezione ogni volta, perché un prodotto live genera dati — chi visita, chi clicca, chi paga, chi si lamenta — e i dati ti dicono cosa costruire dopo.

Ogni prodotto di successo per sviluppatori che ho studiato ha seguito questo schema: spedisci veloce, impara veloce, itera veloce. Quelli che hanno fallito? Tutti avevano file README stupendi e zero utenti.

Ecco il tuo playbook minuto per minuto.

### Giorno 1 — Sabato

#### Blocco Mattina (4 ore): Valida la Domanda

Prima di scrivere una singola riga di codice, ti servono prove che qualcuno oltre a te voglia questa cosa. Non certezza — prove. La differenza conta. La certezza è impossibile. Le prove sono ottenibili in 4 ore.

**Passo 1: Verifica del Volume di Ricerca (45 minuti)**

Vai a queste fonti e cerca la tua idea di prodotto e i termini correlati:

- **Google Trends** (https://trends.google.com) — Gratuito. Mostra l'interesse di ricerca relativo nel tempo. Vuoi vedere una linea piatta o in salita, non in discesa.
- **Ahrefs Free Webmaster Tools** (https://ahrefs.com/webmaster-tools) — Gratuito con verifica del sito. Mostra i volumi delle keyword.
- **Ubersuggest** (https://neilpatel.com/ubersuggest/) — Il piano gratuito dà 3 ricerche al giorno. Mostra volume di ricerca, difficoltà e termini correlati.
- **AlsoAsked** (https://alsoasked.com) — Piano gratuito. Mostra i dati "Le persone chiedono anche" da Google. Rivela quali domande le persone stanno realmente facendo.

Cosa stai cercando:

```
Segnali POSITIVI:
- 500+ ricerche mensili per la tua keyword principale
- Trend in crescita negli ultimi 12 mesi
- Molteplici domande "Le persone chiedono anche" senza buone risposte
- Keyword long-tail correlate con bassa competizione

Segnali NEGATIVI:
- Interesse di ricerca in calo
- Zero volume di ricerca (nessuno sta cercando questo)
- Dominato da aziende enormi nella prima pagina
- Nessuna variazione nei termini di ricerca (troppo ristretto)
```

Esempio reale: supponi che la tua idea dal Modulo R per il motore di revenue sia una "libreria di componenti Tailwind CSS per dashboard SaaS."

```
Ricerca: "tailwind dashboard components" — 2.900/mese, trend in crescita
Ricerca: "tailwind admin template" — 6.600/mese, stabile
Ricerca: "react dashboard template tailwind" — 1.300/mese, in crescita
Correlati: "shadcn dashboard", "tailwind analytics components"

Verdetto: Domanda forte. Molteplici angoli di keyword. Procedi.
```

Altro esempio: supponi che la tua idea sia un "tool CLI in Rust per anonimizzare file di log."

```
Ricerca: "log file anonymizer" — 90/mese, piatto
Ricerca: "anonymize log files" — 140/mese, piatto
Ricerca: "PII removal from logs" — 320/mese, in crescita
Correlati: "GDPR log compliance", "scrub PII from logs"

Verdetto: Nicchia ma in crescita. L'angolo "PII removal" ha più volume
dell'angolo "anonymizer". Riformula il tuo posizionamento.
```

**Passo 2: Mining dei Thread nelle Community (60 minuti)**

Vai dove gli sviluppatori chiedono cose e cerca nel tuo spazio problematico:

- **Reddit:** Cerca in r/webdev, r/reactjs, r/selfhosted, r/SideProject, r/programming, e subreddit di nicchia rilevanti per il tuo dominio
- **Hacker News:** Usa https://hn.algolia.com per cercare nelle discussioni passate
- **GitHub Issues:** Cerca issue in repo popolari correlati al tuo spazio
- **Stack Overflow:** Cerca domande con molti upvote ma risposte accettate insoddisfacenti
- **Server Discord:** Controlla i server delle community di sviluppatori rilevanti

Cosa stai documentando:

```markdown
## Risultati del Thread Mining

### Thread 1
- **Fonte:** Reddit r/reactjs
- **URL:** [link]
- **Titolo:** "Is there a good Tailwind dashboard kit that isn't $200?"
- **Upvote:** 147
- **Commenti:** 83
- **Citazioni chiave:**
  - "Everything on the market is either free and ugly, or $200+ and overkill"
  - "I just need 10-15 well-designed components, not 500"
  - "Would pay $49 for something that actually looks good out of the box"
- **Conclusione:** Sensibilità al prezzo oltre i $200, disponibilità a pagare a $29-49

### Thread 2
- ...
```

Trova almeno 5 thread. Se non riesci a trovare 5 thread dove le persone chiedono qualcosa nello spazio del tuo prodotto, è un serio segnale d'allarme. O la domanda non esiste, o stai cercando con i termini sbagliati. Prova keyword diverse prima di abbandonare l'idea.

**Passo 3: Audit dei Competitor (45 minuti)**

Cerca cosa esiste già. Questo non è scoraggiante — è validante. I competitor significano che c'è un mercato. Nessun competitor di solito significa che non c'è mercato, non che hai trovato un oceano blu.

Per ogni competitor, documenta:

```markdown
## Audit dei Competitor

### Competitor 1: [Nome]
- **URL:** [link]
- **Prezzo:** $XX
- **Cosa fanno bene:** [cose specifiche]
- **Cosa fa schifo:** [lamentele specifiche da recensioni/thread]
- **Le loro recensioni:** [controlla G2, recensioni su ProductHunt, menzioni su Reddit]
- **Il tuo angolo:** [come lo faresti diversamente]

### Competitor 2: [Nome]
- ...
```

L'oro è in "cosa fa schifo." Ogni lamentela su un competitor è una feature request per il tuo prodotto. Le persone ti stanno letteralmente dicendo cosa costruire e quanto far pagare.

**Passo 4: Il Test "10 Persone Pagherebbero" (30 minuti)**

Questo è il gate di validazione finale. Devi trovare prove che almeno 10 persone pagherebbero per questo. Non "hanno espresso interesse." Non "hanno detto che era figo." Pagherebbero.

Fonti di prove:
- Thread su Reddit dove le persone dicono "pagherei per X" (segnale più forte)
- Prodotti competitor con clienti paganti (dimostra che il mercato paga)
- Prodotti su Gumroad/Lemon Squeezy nel tuo spazio con conteggi di vendite visibili
- Repository GitHub con 1.000+ stelle che risolvono un problema correlato (le persone lo valutano abbastanza da mettere una stella)
- Il tuo stesso pubblico se ne hai uno (twitta, scrivi in DM a 10 persone, chiedi direttamente)

Se superi questo test: procedi. Costruiscilo.

Se non superi questo test: cambia il tuo angolo, non l'intera idea. La domanda potrebbe esistere in uno spazio adiacente. Prova un posizionamento diverso prima di abbandonare.

> **Parliamoci Chiaro:** La maggior parte degli sviluppatori salta completamente la validazione perché vuole programmare. Passeranno 200 ore a costruire qualcosa che nessuno ha chiesto, e poi si chiederanno perché nessuno lo compra. Queste 4 ore di ricerca ti faranno risparmiare 196 ore di sforzo sprecato. Non saltarle. Il codice è la parte facile.

#### Blocco Pomeriggio (4 ore): Costruisci l'MVP

Hai validato la domanda. Hai la ricerca sui competitor. Sai cosa vogliono le persone e cosa manca alle soluzioni esistenti. Adesso costruisci la versione minima che risolve il problema centrale.

{? if profile.gpu.exists ?}
Con una GPU nel tuo setup ({= profile.gpu.model | fallback("la tua GPU") =}), considera idee di prodotto che sfruttano l'inferenza AI locale — tool di elaborazione immagini, utility di analisi del codice, pipeline di generazione contenuti. Le feature potenziate da GPU sono un vero differenziatore che la maggior parte degli sviluppatori indie non può offrire.
{? endif ?}

**La Regola delle 3 Feature**

La tua v0.1 ha esattamente 3 feature. Non 4. Non 7. Tre.

Come sceglierle:
1. Qual è la COSA che il tuo prodotto fa? (Feature 1 — il core)
2. Cosa lo rende usabile? (Feature 2 — di solito autenticazione, o salvataggio/esportazione, o configurazione)
3. Cosa lo rende degno di essere pagato rispetto alle alternative? (Feature 3 — il tuo differenziatore)

Tutto il resto va in una lista "v0.2" che non tocchi questo weekend.

Esempio reale — una libreria di componenti Tailwind per dashboard:
1. **Core:** 12 componenti dashboard production-ready (grafici, tabelle, schede statistiche, navigazione)
2. **Usabile:** Snippet di codice copia-e-incolla con anteprima live
3. **Differenziatore:** Dark mode integrata, componenti progettati per funzionare insieme (non una raccolta casuale)

Esempio reale — un tool CLI per la pulizia PII dai log:
1. **Core:** Rileva e oscura PII dai file di log (email, IP, nomi, codici fiscali)
2. **Usabile:** Funziona come pipe CLI (`cat logs.txt | pii-scrub > clean.txt`)
3. **Differenziatore:** File di regole configurabile, gestisce 15+ formati di log automaticamente

{@ insight stack_fit @}

**Scaffolding del Progetto**

Usa gli LLM per accelerare, non sostituire, il tuo lavoro. Ecco il workflow pratico:

{? if stack.contains("react") ?}
Dato che il tuo stack principale include React, lo scaffold per web app qui sotto è il tuo percorso più veloce. Conosci già i tool — concentra le tue 48 ore sulla logica di prodotto, non sull'imparare un nuovo framework.
{? elif stack.contains("rust") ?}
Dato che il tuo stack principale include Rust, lo scaffold per tool CLI qui sotto è il tuo percorso più veloce. I tool CLI in Rust hanno un'eccellente distribuzione (singolo binario, cross-platform) e il pubblico degli sviluppatori rispetta la storia delle performance.
{? elif stack.contains("python") ?}
Dato che il tuo stack principale include Python, considera un tool CLI o un servizio API. Python spedisce veloce con FastAPI o Typer, e l'ecosistema PyPI ti dà distribuzione istantanea a milioni di sviluppatori.
{? endif ?}

```bash
# Scaffold per una web app (tool SaaS, libreria componenti con sito docs, ecc.)
pnpm create vite@latest my-product -- --template react-ts
cd my-product
pnpm install

# Aggiungi Tailwind CSS (il più comune per prodotti per sviluppatori)
pnpm install -D tailwindcss @tailwindcss/vite

# Aggiungi routing se ti servono più pagine
pnpm install react-router-dom

# Struttura del progetto — tienila piatta per un build da 48 ore
mkdir -p src/components src/pages src/lib
```

```bash
# Scaffold per un tool CLI (per utility da sviluppatore)
cargo init my-tool
cd my-tool

# Dipendenze comuni per tool CLI
cargo add clap --features derive    # Parsing degli argomenti
cargo add serde --features derive   # Serializzazione
cargo add serde_json                # Gestione JSON
cargo add anyhow                    # Gestione errori
cargo add regex                     # Pattern matching
```

```bash
# Scaffold per un pacchetto npm (per librerie/utility)
mkdir my-package && cd my-package
pnpm init
pnpm install -D typescript tsup vitest
mkdir src
```

**Il Workflow LLM per Costruire**

{? if settings.has_llm ?}
Hai un LLM configurato ({= settings.llm_provider | fallback("locale") =} / {= settings.llm_model | fallback("il tuo modello") =}). Usalo come pair programmer durante lo sprint — accelera significativamente lo scaffolding e la generazione di boilerplate.
{? endif ?}

Non chiedere all'LLM di costruire l'intero prodotto. Questo produce codice generico e fragile. Invece:

1. **Tu** scrivi l'architettura: struttura dei file, flusso dei dati, interfacce chiave
2. **LLM** genera il boilerplate: componenti ripetitivi, funzioni utility, definizioni di tipo
3. **Tu** scrivi la logica core: la parte che rende il tuo prodotto diverso
4. **LLM** genera i test: test unitari, edge case, test di integrazione
5. **Tu** rivedi e modifichi tutto: il tuo nome è su questo prodotto

Lavoro parallelo mentre programmi: apri una seconda chat con l'LLM e fagli scrivere una bozza del copy della landing page, README e documentazione. Li modificherai di sera, ma le prime bozze saranno pronte.

**Disciplina del Tempo**

```
14:00 — Feature 1 (funzionalità core): 2 ore
         Se non funziona entro le 16:00, taglia lo scope.
16:00 — Feature 2 (usabilità): 1 ora
         Mantienila semplice. La rifinitura arriva dopo.
17:00 — Feature 3 (differenziatore): 1 ora
         Questo è ciò che ti rende degno di essere pagato. Concentrati qui.
18:00 — SMETTI DI PROGRAMMARE. Non deve essere perfetto.
```

> **Errore Comune:** "Solo un'altra feature prima di fermarmi." Così i progetti del weekend diventano progetti mensili. Le 3 feature sono il tuo scope. Se ti viene una grande idea durante il build, scrivila nella lista v0.2 e vai avanti. Puoi aggiungerla la prossima settimana dopo aver avuto clienti paganti.

#### Blocco Sera (2 ore): Scrivi la Landing Page

La tua landing page ha un solo lavoro: convincere un visitatore a pagare. Non deve essere bella. Deve essere chiara.

**La Landing Page in 5 Sezioni**

Ogni landing page di successo per prodotti per sviluppatori segue questa struttura. Non reinventarla:

```
Sezione 1: TITOLO + SOTTOTITOLO
  - Cosa fa in 8 parole o meno
  - Per chi è e quale risultato ottengono

Sezione 2: IL PROBLEMA
  - 3 pain point che il tuo cliente target riconosce
  - Usa il loro linguaggio esatto dal thread mining

Sezione 3: LA SOLUZIONE
  - Screenshot o esempi di codice del tuo prodotto
  - 3 feature mappate sui 3 pain point sopra

Sezione 4: PRICING
  - Uno o due livelli. Mantienilo semplice per la v0.1.
  - Opzione di fatturazione annuale se è un abbonamento.

Sezione 5: CTA (Call to Action)
  - Un pulsante. "Inizia", "Acquista Ora", "Scarica".
  - Ripeti il beneficio principale.
```

**Esempio di Copy Reale — Kit Dashboard Tailwind:**

```markdown
# Sezione 1
## DashKit — Componenti Dashboard Tailwind Production-Ready
Spedisci la dashboard del tuo SaaS in ore, non settimane.
12 componenti copia-e-incolla. Dark mode. $29.

# Sezione 2
## Il Problema
- I kit UI generici ti danno 500 componenti ma zero coerenza
- Costruire UI per dashboard da zero richiede 40+ ore
- Le opzioni gratuite sembrano Bootstrap del 2018

# Sezione 3
## Cosa Ottieni
- **12 componenti** progettati per funzionare insieme (non una raccolta casuale)
- **Dark mode** integrata — attiva/disattiva con una prop
- **Codice copia-e-incolla** — niente npm install, niente dipendenze, niente lock-in
[screenshot degli esempi di componenti]

# Sezione 4
## Pricing
**DashKit** — $29 una tantum
- Tutti i 12 componenti con codice sorgente
- Aggiornamenti gratuiti per 12 mesi
- Usa in progetti illimitati

**DashKit Pro** — $59 una tantum
- Tutto ciò che è in DashKit
- 8 template pagina completa (analytics, CRM, admin, impostazioni)
- File di design Figma
- Richieste di feature prioritarie

# Sezione 5
## Spedisci la tua dashboard questo weekend.
[Acquista DashKit — $29]
```

**Esempio di Copy Reale — PII Log Scrubber:**

```markdown
# Sezione 1
## ScrubLog — Rimuovi i PII dai File di Log in Secondi
Conformità GDPR per i tuoi log. Un solo comando.

# Sezione 2
## Il Problema
- I tuoi log contengono email, IP e nomi che non dovresti conservare
- L'oscuramento manuale richiede ore e dimentica cose
- I tool enterprise costano $500/mese e richiedono un dottorato per configurarli

# Sezione 3
## Come Funziona
```bash
cat server.log | scrublog > clean.log
```
- Rileva 15+ pattern PII automaticamente
- Regole personalizzate via configurazione YAML
- Gestisce formati JSON, Apache, Nginx e testo semplice
[screenshot del terminale che mostra prima/dopo]

# Sezione 4
## Pricing
**Personal** — Gratuito
- 5 pattern PII, 1 formato di log

**Pro** — $19/mese
- Tutti i 15+ pattern PII
- Tutti i formati di log
- Regole personalizzate
- Condivisione configurazione nel team

# Sezione 5
## Smetti di conservare PII di cui non hai bisogno.
[Ottieni ScrubLog Pro — $19/mese]
```

**Workflow LLM per il Copy:**

1. Dai all'LLM il tuo audit dei competitor e i risultati del thread mining
2. Chiedigli di scrivere una bozza del copy della landing page usando il template a 5 sezioni
3. Modifica senza pietà: sostituisci ogni frase vaga con una specifica
4. Leggilo ad alta voce. Se qualche frase ti fa rabbrividire, riscrivila.

**Costruire la Landing Page:**

Per uno sprint di 48 ore, non costruire una landing page personalizzata da zero. Usa una di queste:

{? if stack.contains("react") ?}
- **La tua app React** — Dato che lavori in React, fai della landing page la homepage da non loggato della tua app o aggiungi una route marketing in Next.js. Zero costo di context-switching.
{? endif ?}
- **Il sito del tuo stesso prodotto** — Se è una web app, fai della landing page la homepage da non loggato
- **Astro + Tailwind** — Sito statico, deploy su Vercel in 2 minuti, estremamente veloce
- **Next.js** — Se il tuo prodotto è già in React, aggiungi una route per la pagina marketing
- **Framer** (https://framer.com) — Builder visuale, esporta codice pulito, piano gratuito disponibile
- **Carrd** (https://carrd.co) — $19/anno, siti a pagina singola semplicissimi

```bash
# Il percorso più veloce: sito statico Astro
pnpm create astro@latest my-product-site
cd my-product-site
pnpm install
# Aggiungi Tailwind
pnpm astro add tailwind
```

Dovresti avere una landing page con il copy entro la fine di sabato. Non servono illustrazioni personalizzate. Non servono animazioni. Servono parole chiare e un pulsante di acquisto.

### Giorno 2 — Domenica

#### Blocco Mattina (3 ore): Deploy

Il tuo prodotto deve essere live su internet a un URL reale. Non localhost. Non un URL di anteprima Vercel con un hash casuale. Un dominio reale, con HTTPS, che puoi condividere e le persone possono visitare.

**Passo 1: Deploy dell'Applicazione (60 minuti)**

{? if computed.os_family == "windows" ?}
Dato che sei su Windows, assicurati che WSL2 sia disponibile se i tuoi tool di deployment lo richiedono. La maggior parte dei tool CLI di deployment (Vercel, Fly.io) funziona nativamente su Windows, ma alcuni script presumono percorsi Unix.
{? elif computed.os_family == "macos" ?}
Su macOS, tutti i CLI di deployment si installano senza problemi tramite Homebrew o download diretto. Sei nel percorso di deployment più fluido.
{? elif computed.os_family == "linux" ?}
Su Linux, hai l'ambiente di deployment più flessibile. Tutti i tool CLI funzionano nativamente, e puoi anche fare self-hosting sulla tua stessa macchina se hai un IP statico e vuoi risparmiare sui costi di hosting.
{? endif ?}

Scegli la piattaforma di deployment in base a cosa hai costruito:

**Sito statico / SPA (libreria componenti, landing page, sito docs):**
```bash
# Vercel — il percorso più veloce per siti statici e Next.js
pnpm install -g vercel
vercel

# Ti farà delle domande. Rispondi sì a tutto.
# Il tuo sito è live in ~60 secondi.
```

**Web app con un backend (tool SaaS, servizio API):**
```bash
# Railway — semplice, buon piano gratuito, gestisce database
# https://railway.app
# Connetti il tuo repo GitHub e fai deploy.

# Oppure Fly.io — più controllo, deployment edge globale
# https://fly.io
curl -L https://fly.io/install.sh | sh
fly launch
fly deploy
```

**Tool CLI / pacchetto npm:**
```bash
# Registry npm
npm publish

# Oppure distribuisci come binario tramite GitHub Releases
# Usa cargo-dist per progetti Rust
cargo install cargo-dist
cargo dist init
cargo dist build
# Carica i binari nella GitHub release
```

**Passo 2: Compra un Dominio (30 minuti)**

Un dominio reale costa $12/anno. Se non puoi investire $12 nel tuo business, non sei serio nel voler avere un business.

**Dove comprare:**
- **Namecheap** (https://namecheap.com) — $8-12/anno per .com, buona gestione DNS
- **Cloudflare Registrar** (https://dash.cloudflare.com) — Prezzi al costo (spesso $9-10/anno per .com), DNS eccellente
- **Porkbun** (https://porkbun.com) — Spesso il più economico per il primo anno, buona UI

**Consigli per la scelta del dominio:**
- Più corto è meglio è. 2 sillabe ideale, 3 massimo.
- `.com` vince ancora per fiducia. `.dev` e `.io` vanno bene per tool per sviluppatori.
- Controlla la disponibilità sul tuo registrar, non su GoDaddy (fanno front-running sulle ricerche).
- Non spendere più di 15 minuti a scegliere. Il nome conta meno di quanto pensi.

```bash
# Punta il tuo dominio a Vercel
# Nel dashboard Vercel: Settings > Domains > Aggiungi il tuo dominio
# Poi nelle impostazioni DNS del tuo registrar, aggiungi:
# Record A: @ -> 76.76.21.21
# Record CNAME: www -> cname.vercel-dns.com

# Oppure se usi Cloudflare per il DNS:
# Aggiungi gli stessi record nel pannello DNS di Cloudflare
# SSL è automatico sia con Vercel che con Cloudflare
```

**Passo 3: Monitoraggio Base (30 minuti)**

Devi sapere due cose: se il sito è online e se le persone lo visitano.

**Monitoraggio uptime (gratuito):**
- **Better Uptime** (https://betteruptime.com) — Il piano gratuito monitora 10 URL ogni 3 minuti
- **UptimeRobot** (https://uptimerobot.com) — Il piano gratuito monitora 50 URL ogni 5 minuti

```
Configura il monitoraggio per:
1. L'URL della tua landing page
2. L'endpoint di salute della tua app (se applicabile)
3. L'URL del webhook dei pagamenti (critico — devi sapere se i pagamenti si rompono)
```

**Analytics (rispettose della privacy):**

Non usare Google Analytics. Il tuo pubblico di sviluppatori lo blocca, è eccessivo per un prodotto nuovo, ed è un rischio per la privacy.

- **Plausible** (https://plausible.io) — $9/mese, privacy-first, script da una riga
- **Fathom** (https://usefathom.com) — $14/mese, privacy-first, leggero
- **Umami** (https://umami.is) — Gratuito e self-hosted, o $9/mese cloud

```html
<!-- Plausible — una riga nel tuo <head> -->
<script defer data-domain="tuodominio.com"
  src="https://plausible.io/js/script.js"></script>

<!-- Umami — una riga nel tuo <head> -->
<script defer
  src="https://tua-istanza-umami.com/script.js"
  data-website-id="tuo-website-id"></script>
```

> **Parliamoci Chiaro:** Sì, $9/mese per l'analytics su un prodotto che non ha ancora fatto soldi sembra non necessario. Ma non puoi migliorare ciò che non puoi misurare. Il primo mese di dati analytics ti dirà di più sul tuo mercato di un mese di congetture. Se $9/mese rompe il tuo budget, fai self-hosting di Umami gratuitamente su Railway.

#### Blocco Pomeriggio (2 ore): Configura i Pagamenti

Se il tuo prodotto non può accettare denaro, è un progetto hobby. Configurare i pagamenti richiede meno tempo di quanto la maggior parte degli sviluppatori pensi — circa 20-30 minuti per il flusso base.

{? if regional.country ?}
> **Processori di pagamento raccomandati per {= regional.country | fallback("il tuo paese") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy, PayPal") =}. Le opzioni qui sotto sono disponibili globalmente, ma verifica che il tuo processore preferito supporti i pagamenti in {= regional.currency | fallback("la tua valuta locale") =}.
{? endif ?}

**Opzione A: Lemon Squeezy (Consigliato per Prodotti Digitali)**

Lemon Squeezy (https://lemonsqueezy.com) gestisce elaborazione pagamenti, tasse sulle vendite, IVA e consegna digitale in un'unica piattaforma. È il percorso più veloce da zero all'accettazione di pagamenti.

Perché Lemon Squeezy invece di Stripe per il tuo primo prodotto:
- Agisce come Merchant of Record — gestiscono per te tasse sulle vendite, IVA e conformità
- Pagine di checkout integrate — nessun lavoro frontend necessario
- Consegna digitale integrata — carica i tuoi file, loro gestiscono l'accesso
- 5% + $0,50 per transazione (più alto di Stripe, ma ti risparmia ore di grattacapi fiscali)

Guida alla configurazione:
1. Registrati su https://app.lemonsqueezy.com
2. Crea uno Store (il nome del tuo business)
3. Aggiungi un Prodotto:
   - Nome, descrizione, prezzo
   - Carica file per la consegna digitale (se applicabile)
   - Configura le chiavi di licenza (se vendi software)
4. Ottieni il tuo URL di checkout — questo è ciò a cui il tuo pulsante "Acquista" punta
5. Configura un webhook per l'automazione post-acquisto

```javascript
// Handler webhook Lemon Squeezy (Node.js/Express)
// POST /api/webhooks/lemonsqueezy

import crypto from 'crypto';

const WEBHOOK_SECRET = process.env.LEMONSQUEEZY_WEBHOOK_SECRET;

export async function handleLemonSqueezyWebhook(req, res) {
  // Verifica la firma del webhook
  const signature = req.headers['x-signature'];
  const hmac = crypto.createHmac('sha256', WEBHOOK_SECRET);
  const digest = hmac.update(JSON.stringify(req.body)).digest('hex');

  if (signature !== digest) {
    return res.status(401).json({ error: 'Invalid signature' });
  }

  const event = req.body;

  switch (event.meta.event_name) {
    case 'order_created': {
      const order = event.data;
      const customerEmail = order.attributes.user_email;
      const productId = order.attributes.first_order_item.product_id;
      const orderId = order.id;

      console.log(`New order: ${orderId} from ${customerEmail}`);

      // Invia email di benvenuto, concedi accesso, crea chiave di licenza, ecc.
      await grantProductAccess(customerEmail, productId);
      await sendWelcomeEmail(customerEmail, orderId);

      break;
    }

    case 'subscription_created': {
      const subscription = event.data;
      const customerEmail = subscription.attributes.user_email;

      console.log(`New subscription from ${customerEmail}`);
      await createSubscription(customerEmail, subscription);

      break;
    }

    case 'subscription_cancelled': {
      const subscription = event.data;
      const customerEmail = subscription.attributes.user_email;

      console.log(`Subscription cancelled: ${customerEmail}`);
      await revokeAccess(customerEmail);

      break;
    }

    default:
      console.log(`Unhandled event: ${event.meta.event_name}`);
  }

  return res.status(200).json({ received: true });
}
```

**Opzione B: Stripe (Più Controllo, Più Lavoro)**

Stripe (https://stripe.com) ti dà più controllo ma richiede che tu gestisca la conformità fiscale separatamente. Meglio per SaaS con fatturazione complessa.

```javascript
// Sessione Stripe Checkout (Node.js)
// Crea una pagina di checkout hostata

import Stripe from 'stripe';

const stripe = new Stripe(process.env.STRIPE_SECRET_KEY);

export async function createCheckoutSession(req, res) {
  const session = await stripe.checkout.sessions.create({
    payment_method_types: ['card'],
    line_items: [
      {
        price_data: {
          currency: 'usd',
          product_data: {
            name: 'DashKit Pro',
            description: '12 Tailwind dashboard components + 8 templates + Figma files',
          },
          unit_amount: 5900, // $59.00 in centesimi
        },
        quantity: 1,
      },
    ],
    mode: 'payment', // 'subscription' per ricorrente
    success_url: `${process.env.DOMAIN}/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${process.env.DOMAIN}/pricing`,
    customer_email: req.body.email, // Pre-compila se ce l'hai
  });

  return res.json({ url: session.url });
}

// Handler webhook Stripe
export async function handleStripeWebhook(req, res) {
  const sig = req.headers['stripe-signature'];

  let event;
  try {
    event = stripe.webhooks.constructEvent(
      req.body, // body grezzo, non JSON parsato
      sig,
      process.env.STRIPE_WEBHOOK_SECRET
    );
  } catch (err) {
    console.error(`Webhook signature verification failed: ${err.message}`);
    return res.status(400).send(`Webhook Error: ${err.message}`);
  }

  switch (event.type) {
    case 'checkout.session.completed': {
      const session = event.data.object;
      await fulfillOrder(session);
      break;
    }
    case 'customer.subscription.deleted': {
      const subscription = event.data.object;
      await revokeSubscriptionAccess(subscription);
      break;
    }
  }

  return res.json({ received: true });
}
```

**Per Entrambe le Piattaforme — Testa Prima del Lancio:**

```bash
# Lemon Squeezy: Usa la modalità test nel dashboard
# Attiva "Test mode" in alto a destra nel dashboard Lemon Squeezy
# Usa il numero di carta: 4242 4242 4242 4242, qualsiasi scadenza futura, qualsiasi CVC

# Stripe: Usa le API key in modalità test
# Carta test: 4242 4242 4242 4242
# Carta test che viene rifiutata: 4000 0000 0000 0002
# Carta test che richiede autenticazione: 4000 0025 0000 3155
```

Esegui l'intero flusso di acquisto tu stesso in modalità test. Clicca il pulsante di acquisto, completa il checkout, verifica che il webhook si attivi, verifica che l'accesso venga concesso. Se qualsiasi passaggio fallisce in modalità test, fallirà per i clienti reali.

> **Errore Comune:** "Configuro i pagamenti dopo, quando avrò qualche utente." Questo è al contrario. Configurare i pagamenti non serve a raccogliere soldi oggi — serve a validare se qualcuno pagherà. Un prodotto senza prezzo è un tool gratuito. Un prodotto con un prezzo è un test di business. Il prezzo stesso è parte della validazione.

#### Blocco Sera (3 ore): Lancio

Il tuo prodotto è live. I pagamenti funzionano. La landing page è chiara. Ora hai bisogno che gli esseri umani la vedano.

**La Strategia di Lancio Soft**

Non fare un "grande lancio" per il tuo primo prodotto. I grandi lanci creano pressione per essere perfetti, e la tua v0.1 non è perfetta. Invece, fai un lancio soft: condividilo in pochi posti, raccogli feedback, correggi i problemi critici, poi fai il grande lancio in 1-2 settimane.

**Piattaforma di Lancio 1: Reddit (30 minuti)**

Pubblica in r/SideProject e in un subreddit di nicchia rilevante per il tuo prodotto.

Template per il post su Reddit:

```markdown
Titolo: I built [cosa fa] in a weekend — [beneficio chiave]

Corpo:
Hey [subreddit],

I've been frustrated with [il problema] for a while, so I built
[nome prodotto] this weekend.

**What it does:**
- [Feature 1 — il valore core]
- [Feature 2]
- [Feature 3]

**What makes it different from [competitor]:**
[Un paragrafo onesto sul tuo differenziatore]

**Pricing:**
[Sii trasparente. "$29 one-time" oppure "Free tier + $19/mo Pro"]

I'd love feedback. What am I missing? What would make this
useful for your workflow?

[Link al prodotto]
```

Regole per i post su Reddit:
- Sii genuinamente utile, non commerciale
- Rispondi a ogni singolo commento (non è opzionale)
- Accetta le critiche con grazia — il feedback negativo è il tipo più prezioso
- Non fare astroturfing (upvote falsi, account multipli). Verrai scoperto e bannato.

**Piattaforma di Lancio 2: Hacker News (30 minuti)**

Se il tuo prodotto è tecnico e interessante, pubblica un Show HN. Nella sezione "Dettagli tecnici", menziona il tuo stack ({= stack.primary | fallback("il tuo stack principale") =}) e spiega perché l'hai scelto — i lettori di HN adorano le decisioni tecniche informate.

Template per Show HN:

```markdown
Titolo: Show HN: [Nome Prodotto] – [cosa fa in <70 caratteri]

Corpo:
[Nome prodotto] is [una frase che spiega cosa fa].

I built this because [motivazione genuina — quale problema stavi
risolvendo per te stesso].

Technical details:
- Built with [stack]
- [Decisione tecnica interessante e perché]
- [Cosa rende l'implementazione degna di nota]

Try it: [URL]

Feedback welcome. I'm particularly interested in [domanda specifica per
il pubblico di HN].
```

Consigli per HN:
- Pubblica tra le 7-9 AM orario US Eastern (traffico massimo)
- Il titolo conta più di tutto il resto. Sii specifico e tecnico.
- I lettori di HN rispettano la sostanza tecnica più della rifinitura marketing
- Rispondi ai commenti immediatamente nelle prime 2 ore. La velocità dei commenti influenza il ranking.
- Non implorare upvote. Pubblica e interagisci.

**Piattaforma di Lancio 3: Twitter/X (30 minuti)**

Scrivi un thread di lancio build-in-public:

```
Tweet 1 (Gancio):
I built [prodotto] in 48 hours this weekend.

It [risolve problema specifico] for [pubblico specifico].

Here's what I shipped, what I learned, and the real numbers. Thread:

Tweet 2 (Il Problema):
The problem:
[Descrivi il pain point in 2-3 frasi]
[Includi uno screenshot o esempio di codice che mostra il dolore]

Tweet 3 (La Soluzione):
So I built [nome prodotto].

[Screenshot/GIF del prodotto in azione]

It does three things:
1. [Feature 1]
2. [Feature 2]
3. [Feature 3]

Tweet 4 (Dettaglio Tecnico):
Tech stack for the nerds:
- [Frontend]
- [Backend]
- [Hosting — menziona la piattaforma specifica]
- [Pagamenti — menziona Lemon Squeezy/Stripe]
- Total cost to run: $XX/month

Tweet 5 (Pricing):
Pricing:
[Pricing chiaro, come sulla landing page]
[Link al prodotto]

Tweet 6 (Richiesta):
Would love feedback from anyone who [descrivi l'utente target].

What am I missing? What would make this a must-have for you?
```

**Piattaforma di Lancio 4: Community Rilevanti (30 minuti)**

Identifica 2-3 community dove il tuo pubblico target si ritrova:

- Server Discord (community di sviluppatori, server specifici per framework)
- Community Slack (molte community di sviluppatori di nicchia hanno gruppi Slack)
- Dev.to / Hashnode (scrivi un breve post "Ho costruito questo")
- Indie Hackers (https://indiehackers.com) — progettato specificamente per questo
- Gruppi Telegram o WhatsApp rilevanti

**Prime 48 Ore Dopo il Lancio — Cosa Monitorare:**

```
Metriche da tracciare:
1. Visitatori unici (dall'analytics)
2. Tasso di click landing page → checkout (dovrebbe essere 2-5%)
3. Tasso di conversione checkout → acquisto (dovrebbe essere 1-3%)
4. Bounce rate (sopra l'80% significa che il titolo/hero è sbagliato)
5. Sorgenti di traffico (da dove vengono i tuoi visitatori?)
6. Commenti e feedback (qualitativo — cosa dicono le persone?)

Calcoli di esempio:
- 500 visitatori in 48 ore (ragionevole da Reddit + HN + Twitter)
- 3% clicca "Acquista" = 15 visite al checkout
- 10% completa l'acquisto = 1-2 vendite
- A $29/vendita = $29-58 nel tuo primo weekend

Non sono soldi da pensione. Sono soldi di VALIDAZIONE.
$29 da uno sconosciuto su internet provano che il tuo prodotto ha valore.
```

Non farti prendere dal panico se ottieni zero vendite nelle prime 48 ore. Guarda il tuo funnel:
- Zero visitatori? La tua distribuzione è il problema, non il tuo prodotto.
- Visitatori ma zero click su "Acquista"? Il tuo copy o il tuo prezzo è il problema.
- Click su "Acquista" ma zero completamenti? Il tuo flusso di checkout è rotto o il tuo prezzo è troppo alto per il valore percepito.

Ognuno di questi ha una soluzione diversa. Ecco perché le metriche contano.

### Tocca a Te

1. **Blocca il tempo.** Apri il tuo calendario adesso e blocca il prossimo sabato dalle 8 alle 20 e la prossima domenica dalle 8 alle 20. Chiamalo "Sprint 48 Ore." Trattalo come un volo che non puoi riprogrammare.

2. **Scegli la tua idea.** Scegli un motore di revenue dal Modulo R. Scrivi lo scope di 3 feature per la tua v0.1. Se non riesci a sceglierne uno, scegli quello che potresti spiegare a un non-sviluppatore in una frase.
{? if dna.primary_stack ?}
   Il tuo percorso di esecuzione più forte è costruire qualcosa con {= dna.primary_stack | fallback("il tuo stack principale") =} — spedisci più veloce dove hai già una competenza profonda.
{? endif ?}

3. **Pre-lavoro.** Prima di sabato, crea account su:
   - Vercel, Railway o Fly.io (deployment)
   - Lemon Squeezy o Stripe (pagamenti)
   - Namecheap, Cloudflare o Porkbun (dominio)
   - Plausible, Fathom o Umami (analytics)
   - Better Uptime o UptimeRobot (monitoraggio)

   Fallo in una sera infrasettimanale così sabato è puro building, non creazione di account.

4. **Prepara le tue piattaforme di lancio.** Se non hai un account Reddit con un po' di karma, inizia a partecipare nei subreddit rilevanti questa settimana. Gli account che pubblicano solo autopromozione vengono segnalati. Se non hai un account Hacker News, creane uno e partecipa a qualche discussione prima.

---

## Lezione 2: La Mentalità "Spedisci, Poi Migliora"

*"La v0.1 con 3 feature batte la v1.0 che non viene mai spedita."*

### La Trappola del Perfezionismo

Gli sviluppatori sono particolarmente suscettibili a una specifica modalità di fallimento: costruire in privato per sempre. Sappiamo com'è il "buon codice". Sappiamo che la nostra v0.1 non è buon codice. Quindi rifattorizziamo. Aggiungiamo gestione degli errori. Scriviamo più test. Miglioriamo l'architettura. Facciamo tutto tranne l'unica cosa che conta: mostrarlo agli esseri umani.

Ecco una verità che ti farà risparmiare migliaia di ore: **i tuoi clienti non leggono il tuo codice sorgente.** Non gli importa della tua architettura. Non gli importa della tua copertura dei test. Gli importa di una sola cosa: questo risolve il mio problema?

Un prodotto con codice spaghetti che risolve un problema reale farà soldi. Un prodotto con architettura elegante che non risolve nessun problema non farà nulla.

Questa non è una scusa per scrivere codice pessimo. È una dichiarazione di priorità. Spedisci prima. Rifattorizza dopo. La rifattorizzazione sarà comunque meglio informata dai dati di utilizzo reale.

### Come Funziona in Pratica "Spedisci, Poi Migliora"

Considera questo scenario: uno sviluppatore lancia un pacchetto di template Notion per manager di ingegneria del software. Ecco come appare al lancio:

- 5 template (non 50)
- Una pagina Gumroad con un paragrafo di descrizione e 3 screenshot
- Nessun sito web personalizzato
- Nessuna mailing list
- Nessun seguito sui social media
- Prezzo: $29

Lo pubblicano su Reddit e Twitter. Questa è l'intera strategia di marketing.

Risultati del mese 1:
- ~170 vendite a $29 = ~$5.000
- Dopo la commissione di Gumroad (10%): ~$4.500
- Tempo investito: ~30 ore totali (costruzione template + scrittura descrizioni)
- Tariffa oraria effettiva: ~$150/ora

Era "perfetto"? No. I template avevano inconsistenze nella formattazione. Alcune descrizioni erano generiche. Ai clienti non importava. Importava loro che risparmiava dal costruire i template da soli.

Entro il mese 3, basandosi sul feedback dei clienti, lo sviluppatore:
- Ha corretto i problemi di formattazione
- Ha aggiunto più template (quelli che i clienti avevano specificamente richiesto)
- Ha alzato il prezzo a $39 (i clienti esistenti hanno ricevuto gli aggiornamenti gratis)
- Ha creato un livello "Pro" con un video walkthrough accompagnatorio

Il prodotto che hanno lanciato era peggiore sotto ogni aspetto rispetto al prodotto che avevano 90 giorni dopo. Ma la versione a 90 giorni è esistita solo perché la versione di lancio ha generato il feedback e le entrate per guidare lo sviluppo.

> **NOTA:** Per una validazione reale del modello "spedisci brutto, migliora veloce": Josh Comeau ha pre-venduto $550K del suo corso CSS for JavaScript Developers nella prima settimana (Fonte: failory.com). Wes Bos ha generato $10M+ in vendite totali di corsi per sviluppatori usando lanci iterativi (Fonte: foundershut.com). Entrambi sono partiti con prodotti v1 imperfetti e hanno iterato basandosi sul feedback reale dei clienti.

### I Primi 10 Clienti Ti Dicono Tutto

I tuoi primi 10 clienti paganti sono le persone più importanti nel tuo business. Non per i loro soldi — 10 vendite a $29 sono $290, che ti comprano la spesa. Sono importanti perché sono volontari per il team di sviluppo del tuo prodotto.

Cosa fare con i tuoi primi 10 clienti:

1. **Invia un'email di ringraziamento personale.** Non automatizzata. Personale. "Ciao, ho visto che hai acquistato [prodotto]. Grazie. Lo sto sviluppando attivamente — c'è qualcosa che vorresti facesse che non fa?"

2. **Leggi ogni risposta.** Alcuni non risponderanno. Alcuni risponderanno con "sembra ottimo, grazie." Ma 2-3 su 10 scriveranno paragrafi su quello che vogliono. Quei paragrafi sono la tua roadmap.

3. **Cerca i pattern.** Se 3 su 10 persone chiedono la stessa feature, costruiscila. Quello è un segnale di domanda del 30% da clienti paganti. Nessun sondaggio ti darà dati così buoni.

4. **Chiedi della loro disponibilità a pagare di più.** "Sto pianificando un livello Pro con [feature X]. Varrebbe $49 per te?" Diretto. Specifico. Ti dà dati sul pricing.

```
Template email per i primi 10 clienti:

Oggetto: Domanda veloce su [nome prodotto]

Ciao [nome],

Ho visto che hai preso [nome prodotto] — grazie per essere
tra i primi clienti.

Lo sto costruendo attivamente e rilascio aggiornamenti settimanali.
Domanda veloce: qual è la COSA che vorresti facesse che
non fa?

Non ci sono risposte sbagliate. Anche se sembra una richiesta
grossa, voglio sentirla.

Grazie,
[Il tuo nome]
```

### Come Gestire il Feedback Negativo

Il tuo primo pezzo di feedback negativo sembrerà personale. Non è personale. È un dato.

**Framework per processare il feedback negativo:**

```
1. PAUSA. Non rispondere per 30 minuti. La tua reazione emotiva
   non è utile.

2. CATEGORIZZA il feedback:
   a) Bug report — correggilo. Ringraziali.
   b) Feature request — aggiungila al backlog. Ringraziali.
   c) Lamentela sul prezzo — annotala. Controlla se è un pattern.
   d) Lamentela sulla qualità — investiga. È valida?
   e) Troll/irragionevole — ignora. Vai avanti.

3. RISPONDI (solo per a, b, c, d):
   "Grazie per il feedback. [Riconosci il problema specifico].
   Sto [correggendo ora / aggiungendo alla roadmap / investigando].
   Ti farò sapere quando sarà risolto."

4. AGISCI. Se hai promesso di correggere qualcosa, correggilo entro una settimana.
   Niente costruisce lealtà più velocemente del mostrare ai clienti che il loro
   feedback porta a cambiamenti reali.
```

> **Parliamoci Chiaro:** Qualcuno dirà che il tuo prodotto è spazzatura. Farà male. Ma se il tuo prodotto è live e fa soldi, hai già fatto qualcosa che la maggior parte degli sviluppatori non fa mai. La persona che critica dai commenti non ha spedito nulla. Tu sì. Continua a spedire.

### Il Ciclo di Iterazione Settimanale

Dopo il lancio, il tuo workflow diventa un ciclo stretto:

```
Lunedì:    Rivedi le metriche e il feedback dei clienti della scorsa settimana
Martedì:   Pianifica il miglioramento della settimana (UNA cosa, non cinque)
Mercoledì: Costruisci il miglioramento
Giovedì:   Testa e deploya il miglioramento
Venerdì:   Scrivi un changelog/post di aggiornamento
Weekend:   Marketing — un post sul blog, un post social, un'interazione con la community

Ripeti.
```

La parola chiave è UN miglioramento a settimana. Non una revisione delle feature. Non un redesign. Una cosa che rende il prodotto leggermente migliore per i tuoi clienti esistenti. In 12 settimane, sono 12 miglioramenti guidati da dati di utilizzo reale. Il tuo prodotto dopo 12 settimane di questo ciclo sarà drasticamente migliore di qualsiasi cosa avresti potuto progettare in isolamento.

### Le Entrate Validano Più Velocemente dei Sondaggi

I sondaggi mentono. Non intenzionalmente — le persone sono semplicemente pessime nel prevedere il proprio comportamento. "Pagheresti $29 per questo?" ottiene facili "sì." Ma "ecco la pagina di checkout, inserisci la tua carta di credito" ottiene risposte oneste.

Ecco perché lanci con i pagamenti dal giorno uno:

| Metodo di Validazione | Tempo per il Segnale | Qualità del Segnale |
|---|---|---|
| Sondaggio / poll | 1-2 settimane | Bassa (le persone mentono) |
| Landing page con iscrizione email | 1-2 settimane | Media (interesse, non impegno) |
| Landing page con prezzo ma senza checkout | 1 settimana | Medio-Alta (accettazione del prezzo) |
| **Prodotto live con checkout reale** | **48 ore** | **La più alta (comportamento d'acquisto reale)** |

Il prezzo di $0 non rivela nulla. Il prezzo di $29 rivela tutto.

### Tocca a Te

1. **Scrivi il tuo impegno per il "lancio brutto."** Apri un file di testo e scrivi: "Lancerò [nome prodotto] il [data] anche se non è perfetto. Scope della v0.1: [3 feature]. Non aggiungerò la Feature 4 prima del lancio." Firmalo (metaforicamente). Consultalo quando ti prende la voglia di rifinire.

2. **Scrivi la tua email per i primi 10 clienti.** Scrivi il template dell'email di ringraziamento personale adesso, prima di avere clienti. Quando arriva la prima vendita, vuoi inviarla entro l'ora.

3. **Configura il tuo tracker di iterazione.** Crea un semplice foglio di calcolo o una pagina Notion con colonne: Settimana | Miglioramento Fatto | Impatto sulla Metrica | Feedback dei Clienti. Questo diventa il tuo registro delle decisioni su cosa costruire dopo.

---

## Lezione 3: Psicologia del Pricing per Prodotti per Sviluppatori

*"$0 non è un prezzo. È una trappola."*

### Perché il Gratuito Costa Caro

La verità più controintuitiva nel vendere prodotti per sviluppatori: **gli utenti gratuiti ti costano più dei clienti paganti.**

Gli utenti gratuiti:
- Inviano più richieste di supporto (non hanno nulla in gioco)
- Pretendono più feature (si sentono in diritto perché non pagano)
- Forniscono feedback meno utile ("figo" non è azionabile)
- Hanno tassi di abbandono più alti (non c'è costo di switching)
- Parlano meno del tuo prodotto (le cose gratuite hanno basso valore percepito)

I clienti paganti:
- Sono investiti nel tuo successo (vogliono che il loro acquisto sia stata una buona decisione)
- Forniscono feedback specifico e azionabile (vogliono che il prodotto migliori)
- Sono più facili da trattenere (hanno già deciso di pagare; l'inerzia lavora a tuo favore)
- Consigliano più spesso agli altri (raccomandare qualcosa che hai pagato valida il tuo acquisto)
- Rispettano il tuo tempo (capiscono che stai gestendo un business)

L'unica ragione per offrire un piano gratuito è come meccanismo di lead generation per il piano a pagamento. Se il tuo piano gratuito è abbastanza buono che le persone non fanno mai upgrade, non hai un piano gratuito — hai un prodotto gratuito con un pulsante per le donazioni.

> **Errore Comune:** "Lo rendo gratuito per avere utenti prima, poi faccio pagare dopo." Questo non funziona quasi mai. Gli utenti che attiri a $0 si aspettano $0 per sempre. Quando aggiungi un prezzo, se ne vanno. Gli utenti che avrebbero pagato $29 dal primo giorno non hanno mai trovato il tuo prodotto perché lo avevi posizionato come tool gratuito. Hai attratto il pubblico sbagliato.

{@ insight cost_projection @}

### I Livelli di Prezzo per Prodotti per Sviluppatori

Dopo aver analizzato centinaia di prodotti di successo per sviluppatori, questi punti di prezzo funzionano in modo consistente. Tutti i prezzi sotto sono in USD — se stai prezzando in {= regional.currency | fallback("la tua valuta locale") =}, adattali per il potere d'acquisto locale e le norme di mercato.

**Livello 1: $9-29 — Tool e Utility per Sviluppatori**

I prodotti in questa fascia risolvono un problema specifico e ristretto. Un singolo acquisto, usalo oggi.

```
Esempi:
- Estensione VS Code con feature premium: $9-15
- Tool CLI con feature pro: $15-19
- Tool SaaS a scopo singolo: $9-19/mese
- Piccola libreria di componenti: $19-29
- Estensione DevTools per browser: $9-15

Psicologia dell'acquirente: Territorio dell'acquisto d'impulso. Lo sviluppatore
lo vede, riconosce il problema, lo compra senza chiedere al suo manager.
Nessuna approvazione di budget necessaria. Carta di credito → fatto.

Intuizione chiave: A questo prezzo, la tua landing page deve convertire
in meno di 2 minuti. L'acquirente non leggerà una lunga lista di feature.
Mostra il problema, mostra la soluzione, mostra il prezzo.
```

**Livello 2: $49-99 — Template, Kit e Tool Completi**

I prodotti in questa fascia fanno risparmiare tempo significativo. Componenti multipli che lavorano insieme.

```
Esempi:
- Kit completo di template UI: $49-79
- Boilerplate SaaS con auth, fatturazione, dashboard: $79-99
- Set completo di icone/illustrazioni: $49-69
- Toolkit CLI multi-scopo: $49
- Libreria wrapper API con documentazione estesa: $49-79

Psicologia dell'acquirente: Acquisto ponderato. Lo sviluppatore valuta
per 5-10 minuti. Confronta con le alternative. Calcola il tempo risparmiato.
"Se questo mi fa risparmiare 10 ore e valuto il mio tempo $50/ora,
$79 è una scelta ovvia."

Intuizione chiave: Ti serve un punto di confronto. Mostra il tempo/sforzo
necessario per costruire questo da zero vs. comprare il tuo kit.
Includi testimonianze se ne hai.
```

**Livello 3: $149-499 — Corsi, Soluzioni Complete, Template Premium**

I prodotti in questa fascia trasformano una competenza o forniscono un sistema completo.

```
Esempi:
- Video corso (10+ ore): $149-299
- Kit starter SaaS con codice sorgente completo + video walkthrough: $199-299
- Libreria di componenti enterprise: $299-499
- Toolkit completo per sviluppatori (tool multipli): $199
- "Costruisci X da Zero" codebase completa + lezioni: $149-249

Psicologia dell'acquirente: Acquisto come investimento. L'acquirente deve
giustificare la spesa (a se stesso o al suo manager). Ha bisogno di
prova sociale, anteprime dettagliate e una narrativa ROI chiara.

Intuizione chiave: A questo livello, offri una garanzia di rimborso.
Riduce l'ansia d'acquisto e aumenta le conversioni. I tassi di rimborso
per prodotti digitali per sviluppatori sono tipicamente 3-5%.
Le conversioni aumentate superano di gran lunga i rimborsi.
```

### La Strategia di Pricing a 3 Livelli

Se il tuo prodotto lo supporta, offri tre livelli di prezzo. Non è casuale — sfrutta un bias cognitivo ben documentato chiamato "effetto palcoscenico centrale." Quando vengono presentate tre opzioni, la maggior parte delle persone sceglie quella di mezzo.

```
Struttura dei livelli:

BASE           PRO (evidenziato)     TEAM/ENTERPRISE
$29             $59                   $149
Feature core    Tutto in Base         Tutto in Pro
                + feature premium     + feature team
                + supporto prioritario + licenza commerciale

Distribuzione delle conversioni (tipica):
- Base: 20-30%
- Pro: 50-60% ← questo è il tuo obiettivo
- Team: 10-20%
```

**Come progettare i livelli:**

1. Inizia con il livello **Pro**. Questo è il prodotto che vuoi realmente vendere, al prezzo che riflette il suo valore. Progetta questo per primo.

2. Crea il livello **Base** rimuovendo feature dal Pro. Rimuovi abbastanza che Base risolva il problema ma Pro lo risolva *bene*. Base dovrebbe risultare leggermente frustrante — usabile, ma chiaramente limitato.

3. Crea il livello **Team** aggiungendo feature al Pro. Licenza multi-posto, diritti d'uso commerciale, supporto prioritario, branding personalizzato, accesso al codice sorgente, file Figma, ecc.

**Esempio reale di pagina pricing:**

```
DashKit

STARTER — $29                    PRO — $59                        TEAM — $149
                                 ★ Più Popolare                   Ideale per agenzie

✓ 12 componenti core            ✓ Tutto in Starter                ✓ Tutto in Pro
✓ React + TypeScript             ✓ 8 template pagina completa      ✓ Fino a 5 membri del team
✓ Dark mode                      ✓ File di design Figma            ✓ Licenza commerciale
✓ npm install                    ✓ Tabella dati avanzata            (progetti cliente illimitati)
✓ 6 mesi di aggiornamenti      ✓ Integrazione libreria grafici   ✓ Supporto prioritario
                                 ✓ 12 mesi di aggiornamenti       ✓ Aggiornamenti a vita
                                 ✓ Richieste feature prioritarie   ✓ Opzioni branding personalizzato

[Prendi Starter]                 [Prendi Pro]                      [Prendi Team]
```

### Ancoraggio del Prezzo

L'ancoraggio è il bias cognitivo per cui il primo numero che le persone vedono influenza la loro percezione dei numeri successivi. Usalo in modo etico:

1. **Mostra l'opzione costosa per prima** (a destra nei layout occidentali). Vedere $149 fa sembrare $59 ragionevole.

2. **Mostra i calcoli delle "ore risparmiate".**
   ```
   "Costruire questi componenti da zero richiede ~40 ore.
   A $50/ora, sono $2.000 del tuo tempo.
   DashKit Pro: $59."
   ```

3. **Usa il reframing "al giorno" per gli abbonamenti.**
   ```
   "$19/mese" → "Meno di $0,63/giorno"
   "$99/anno" → "$8,25/mese" o "$0,27/giorno"
   ```

4. **Sconto fatturazione annuale.** Offri 2 mesi gratis sui piani annuali. Questo è standard e atteso. La fatturazione annuale riduce il churn del 30-40% perché la cancellazione richiede una decisione consapevole in un singolo punto di rinnovo, non una decisione mensile continua.

```
Mensile: $19/mese
Annuale: $190/anno (risparmia $38 — 2 mesi gratis)

Mostra come:
Mensile: $19/mese
Annuale: $15,83/mese (fatturato annualmente a $190)
```

### A/B Testing dei Prezzi

Testare i prezzi è utile ma delicato. Ecco come farlo senza essere disonesti:

**Approcci accettabili:**
- Testa prezzi diversi su canali di lancio diversi (Reddit ottiene $29, Product Hunt ottiene $39, vedi quale converte meglio)
- Cambia il tuo prezzo dopo 2 settimane e confronta i tassi di conversione
- Offri uno sconto di lancio ("$29 questa settimana, $39 dopo") e vedi se l'urgenza cambia il comportamento
- Testa strutture di livelli diverse (2 livelli vs 3 livelli) in periodi di tempo diversi

**Non accettabile:**
- Mostrare prezzi diversi a visitatori diversi sulla stessa pagina nello stesso momento (discriminazione di prezzo, erode la fiducia)
- Far pagare di più in base alla posizione o al rilevamento del browser (le persone parlano, e verrai scoperto)

### Quando Alzare i Prezzi

Alza i tuoi prezzi quando una di queste condizioni è vera:

1. **Il tasso di conversione è sopra il 5%.** Sei troppo economico. Un tasso di conversione salutare per la landing page di un prodotto per sviluppatori è 1-3%. Sopra il 5% significa che quasi tutti quelli che vedono il prezzo concordano che è un buon affare — il che significa che stai lasciando soldi sul tavolo.

2. **Nessuno si è lamentato del prezzo.** Se zero persone su 100 dicono che è troppo caro, è troppo economico. Un prodotto sano ha circa il 20% dei visitatori che pensano che il prezzo sia alto. Ciò significa che l'80% pensa sia giusto o un buon affare.

3. **Hai aggiunto feature significative dal lancio.** Hai lanciato a $29 con 3 feature. Ora ne hai 8 e documentazione migliore. Il prodotto vale di più. Fai pagare di più.

4. **Hai testimonianze e prova sociale.** Il valore percepito aumenta con la prova sociale. Una volta che hai 5+ recensioni positive, il tuo prodotto vale di più nella mente dell'acquirente.

**Come alzare i prezzi:**
- Annuncia l'aumento di prezzo 1-2 settimane in anticipo ("Il prezzo passa da $29 a $39 il [data]")
- Mantieni il vecchio prezzo per i clienti esistenti
- Non è scorretto — è una pratica standard e crea anche urgenza per gli indecisi

> **Parliamoci Chiaro:** La maggior parte degli sviluppatori sottoprezza del 50-200%. Il tuo prodotto da {= regional.currency_symbol | fallback("$") =}29 probabilmente vale {= regional.currency_symbol | fallback("$") =}49. Il tuo prodotto da {= regional.currency_symbol | fallback("$") =}49 probabilmente vale {= regional.currency_symbol | fallback("$") =}79. Lo so perché gli sviluppatori ancorano alla propria disponibilità a pagare (bassa — siamo tirchi sugli strumenti) piuttosto che alla disponibilità del cliente a pagare (più alta — stanno comprando una soluzione a un problema che gli costa tempo). Alza i tuoi prezzi prima di quanto pensi.

### Tocca a Te

1. **Prezza il tuo prodotto.** In base all'analisi dei livelli sopra, scegli un punto di prezzo per il lancio della tua v0.1. Scrivilo. Se ti senti a disagio perché sembra "troppo alto," sei probabilmente nella fascia giusta. Se ti sembra comodo, aggiungi il 50%.

2. **Progetta la tua pagina pricing.** Usando il template a 3 livelli, progetta il copy della tua pagina pricing. Identifica quali feature vanno in ogni livello. Identifica il tuo livello "evidenziato" (quello che vuoi che la maggior parte delle persone compri).

3. **Calcola i tuoi numeri.** Compila:
   - Prezzo per vendita: {= regional.currency_symbol | fallback("$") =}___
   - Obiettivo di entrate mensili: {= regional.currency_symbol | fallback("$") =}___
   - Numero di vendite necessarie al mese: ___
   - Visitatori stimati della landing page necessari (al 2% di conversione): ___
   - Quel numero di visitatori è raggiungibile con il tuo piano di distribuzione? (Sì/No)

---

## Lezione 4: Setup Legale Minimo Viabile

*"30 minuti di setup legale adesso ti risparmiano 30 ore di panico dopo."*

### La Verità Onesta sul Setup Legale

La maggior parte degli sviluppatori o ignora completamente il lato legale (rischioso) o ne rimane paralizzata (sprecato). L'approccio giusto è un setup legale minimo viabile: abbastanza protezione per operare legittimamente, senza spendere $5.000 per un avvocato prima di aver guadagnato $5.

Ecco cosa ti serve davvero prima della tua prima vendita, cosa ti serve prima della tua centesima vendita, e cosa non ti serve fino a molto più tardi.

### Prima della Tua Prima Vendita (Fallo Questo Weekend)

**1. Controlla il Tuo Contratto di Lavoro (30 minuti)**

Se hai un lavoro a tempo pieno, leggi la clausola sulla proprietà intellettuale del tuo contratto di lavoro prima di costruire qualsiasi cosa. Cerca specificamente:

- **Clausole di cessione delle invenzioni:** Alcuni contratti dicono che tutto ciò che crei mentre sei impiegato — incluso nel tuo tempo libero — appartiene al tuo datore di lavoro.
- **Clausole di non concorrenza:** Alcune ti impediscono di lavorare nello stesso settore, anche come progetto secondario.
- **Politiche sulle attività collaterali:** Alcune richiedono approvazione scritta per attività commerciali esterne.

```
Cosa stai cercando:

SICURO: "Le invenzioni fatte in orario aziendale o usando risorse aziendali
appartengono all'azienda." → Il tuo progetto del weekend sulla tua macchina
personale è tuo.

AMBIGUO: "Tutte le invenzioni relative al business attuale o previsto
dell'azienda." → Se il tuo progetto secondario è nello stesso
dominio del tuo datore di lavoro, chiedi un parere legale.

RESTRITTIVO: "Tutte le invenzioni concepite durante il periodo di
impiego appartengono all'azienda." → Questo è aggressivo ma
comune in alcune aziende. Chiedi un parere legale prima di procedere.
```

Stati come California, Delaware, Illinois, Minnesota, Washington e altri hanno leggi che limitano quanto ampiamente i datori di lavoro possono rivendicare le tue invenzioni personali. Ma il linguaggio specifico del tuo contratto conta.

> **Errore Comune:** "Lo terrò semplicemente segreto." Se il tuo prodotto ha abbastanza successo da importare, qualcuno se ne accorgerà. Se viola il tuo contratto di lavoro, potresti perdere il prodotto E il lavoro. 30 minuti a leggere il tuo contratto adesso prevengono questo.

**2. Informativa sulla Privacy (15 minuti)**

Se il tuo prodotto raccoglie qualsiasi dato — anche solo un indirizzo email per l'acquisto — ti serve un'informativa sulla privacy. È un requisito legale nell'UE (GDPR), in California (CCPA), e sempre più ovunque.

Non scriverne una da zero. Usa un generatore:

- **Termly** (https://termly.io/products/privacy-policy-generator/) — Piano gratuito, rispondi alle domande, ottieni una policy
- **Avodocs** (https://www.avodocs.com) — Gratuito, template legali open-source
- **Iubenda** (https://www.iubenda.com) — Piano gratuito, auto-genera in base al tuo stack tecnologico

La tua informativa sulla privacy deve coprire:

```markdown
# Informativa sulla Privacy per [Nome Prodotto]
Ultimo aggiornamento: [Data]

## Cosa Raccogliamo
- Indirizzo email (per conferma acquisto e aggiornamenti prodotto)
- Informazioni di pagamento (processate da [Lemon Squeezy/Stripe],
  non vediamo né archiviamo mai i dettagli della tua carta)
- Analytics di utilizzo base (visualizzazioni pagina, utilizzo feature — tramite
  [Plausible/Fathom/Umami], rispettose della privacy, niente cookie)

## Cosa NON Raccogliamo
- Non ti tracciamo attraverso il web
- Non vendiamo i tuoi dati a nessuno
- Non usiamo cookie pubblicitari

## Come Usiamo i Tuoi Dati
- Per consegnare il prodotto che hai acquistato
- Per inviare aggiornamenti prodotto e avvisi importanti
- Per migliorare il prodotto in base ai pattern di utilizzo aggregati

## Archiviazione dei Dati
- I tuoi dati sono archiviati sui server di [provider hosting] in [regione]
- I dati di pagamento sono gestiti interamente da [Lemon Squeezy/Stripe]

## I Tuoi Diritti
- Puoi richiedere una copia dei tuoi dati in qualsiasi momento
- Puoi richiedere la cancellazione dei tuoi dati in qualsiasi momento
- Contatto: [la tua email]

## Modifiche
- Ti notificheremo di cambiamenti significativi via email
```

Mettila su `tuodominio.com/privacy`. Linkala dal footer della pagina di checkout.

**3. Termini di Servizio (15 minuti)**

I tuoi termini di servizio ti proteggono da richieste irragionevoli. Per un prodotto digitale, sono semplici.

```markdown
# Termini di Servizio per [Nome Prodotto]
Ultimo aggiornamento: [Data]

## Licenza
Quando acquisti [Nome Prodotto], ricevi una licenza per usarlo
per scopi [personali/commerciali].

- **Licenza singola:** Usa nei tuoi progetti (illimitati)
- **Licenza team:** Uso da parte di fino a [N] membri del team
- NON puoi redistribuire, rivendere o condividere le credenziali di accesso

## Rimborsi
- Prodotti digitali: garanzia di rimborso di [30 giorni / 14 giorni]
- Se non sei soddisfatto, scrivi a [la tua email] per un rimborso completo
- Nessuna domanda entro la finestra di rimborso

## Responsabilità
- [Nome Prodotto] è fornito "così com'è" senza garanzia
- Non siamo responsabili per danni derivanti dall'uso del prodotto
- La responsabilità massima è limitata all'importo che hai pagato

## Supporto
- Il supporto è fornito via email a [la tua email]
- Miriamo a rispondere entro [48 ore / 2 giorni lavorativi]

## Modifiche
- Possiamo aggiornare questi termini con preavviso
- L'uso continuato costituisce accettazione dei termini aggiornati
```

Mettili su `tuodominio.com/terms`. Linkali dal footer della pagina di checkout.

### Prima della Tua Centesima Vendita (Primi Mesi)

**4. Entità Commerciale (1-3 ore + tempo di elaborazione)**

Operare come ditta individuale (il default quando vendi cose senza formare un'azienda) funziona per le tue prime vendite. Ma man mano che le entrate crescono, vorrai protezione dalla responsabilità e vantaggi fiscali.

{? if regional.country ?}
> **Per {= regional.country | fallback("la tua regione") =}:** Il tipo di entità raccomandato è una **{= regional.business_entity_type | fallback("LLC o equivalente") =}**, con costi di registrazione tipici di {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Trova la sezione del tuo paese qui sotto per una guida specifica.
{? endif ?}

**Stati Uniti — LLC:**

Una LLC (Limited Liability Company) è la scelta standard per business da sviluppatore in solitaria.

```
Costo: $50-500 a seconda dello stato (tassa di deposito)
Tempo: 1-4 settimane per l'elaborazione
Dove depositare: Il tuo stato di residenza, a meno che non ci sia
un motivo specifico per usare Delaware o Wyoming

Deposito fai-da-te (più economico):
1. Vai al sito web del Secretary of State del tuo stato
2. Deposita gli "Articles of Organization" (il modulo è solitamente 1-2 pagine)
3. Paga la tassa di deposito ($50-250 a seconda dello stato)
4. Ottieni il tuo EIN (codice fiscale) da IRS.gov — gratuito, istantaneo online

Confronto tra stati per sviluppatori in solitaria:
- Wyoming: $100 deposito, $60/anno report annuale. Nessuna tassa
             sul reddito statale. Buono per la privacy (nessuna info
             pubblica sui membri richiesta).
- Delaware: $90 deposito, $300/anno tassa annuale. Popolare ma non
            necessariamente migliore per sviluppatori in solitaria.
- New Mexico: $50 deposito, nessun report annuale. Il più economico
              da mantenere.
- California: $70 deposito, $800/anno tassa minima di franchise.
              Costoso. Lo paghi anche se guadagni $0.
```

**Stripe Atlas (se vuoi che qualcuno lo faccia per te):**

Stripe Atlas (https://atlas.stripe.com) costa $500 e configura una LLC Delaware, un conto bancario US (tramite Mercury), un account Stripe e fornisce guide fiscali e legali. Se non sei negli US o vuoi semplicemente che qualcun altro gestisca le pratiche, vale i $500.

**Regno Unito — Ltd Company:**

```
Costo: GBP 12 alla Companies House (https://www.gov.uk/set-up-limited-company)
Tempo: Solitamente 24-48 ore
Costi ricorrenti: Dichiarazione di conferma annuale (GBP 13), deposito bilancio annuale

Per sviluppatori in solitaria: Una Ltd company ti dà protezione dalla responsabilità
e efficienza fiscale una volta che i profitti superano ~GBP 50.000/anno.
Sotto quella soglia, la ditta individuale è più semplice.
```

**Unione Europea:**

Ogni paese ha la sua struttura. Opzioni comuni:
- **Germania:** GmbH (costosa da costituire) o registrazione come freelancer (economica)
- **Paesi Bassi:** BV o eenmanszaak (ditta individuale)
- **Francia:** auto-entrepreneur (micro-impresa) — molto comune per sviluppatori in solitaria, tassazione forfettaria semplice
- **Estonia:** e-Residency + OUe estone (popolare con i nomadi digitali, azienda UE completa per ~EUR 190)

**Australia:**

```
Ditta individuale: Registrazione gratuita tramite domanda ABN (https://www.abr.gov.au)
Azienda (Pty Ltd): AUD 538 registrazione con ASIC
Per sviluppatori in solitaria: Inizia come ditta individuale. Registra un'azienda
quando le entrate giustificano il sovraccarico contabile (~AUD 100K+/anno).
```

**5. Obblighi Fiscali**

Se usi Lemon Squeezy come piattaforma di pagamento, gestiscono le tasse sulle vendite e l'IVA come Merchant of Record. Questa è una semplificazione enorme.

Se usi Stripe direttamente, sei responsabile per:
- **Tasse sulle vendite USA:** Variano per stato. Usa Stripe Tax ($0,50/transazione) o TaxJar per automatizzare.
- **IVA UE:** 20-27% a seconda del paese. Obbligatoria per vendite digitali a clienti UE indipendentemente da dove ti trovi. Lemon Squeezy la gestisce; Stripe Tax può automatizzarla.
- **IVA UK:** 20%. Obbligatoria se le tue vendite UK superano GBP 85.000/anno.
- **Tasse sui Servizi Digitali:** Vari paesi le stanno imponendo. Un altro motivo per usare Lemon Squeezy finché il tuo volume non giustifica la gestione autonoma.

{? if regional.country ?}
> **Nota fiscale per {= regional.country | fallback("la tua regione") =}:** {= regional.tax_note | fallback("Consulta un professionista fiscale locale per le specifiche sui tuoi obblighi.") =}
{? endif ?}

> **Parliamoci Chiaro:** Il singolo più grande vantaggio di Lemon Squeezy rispetto a Stripe per uno sviluppatore in solitaria non è la pagina di checkout o le feature. È che gestiscono la conformità fiscale a livello globale. Le tasse sulle vendite internazionali sono un incubo. Lemon Squeezy prende il 5% + $0,50 per transazione e fa sparire l'incubo. Finché non guadagni {= regional.currency_symbol | fallback("$") =}5.000+/mese, il 5% ne vale la pena. Dopo, valuta se gestire le tasse da solo con Stripe + TaxJar ti fa risparmiare denaro e sanità mentale.

**6. Basi di Proprietà Intellettuale**

Cosa devi sapere:

- **Il tuo codice è automaticamente protetto da copyright** nel momento in cui lo scrivi. Nessuna registrazione necessaria. Ma la registrazione (US: $65 su copyright.gov) ti dà una posizione legale più forte nelle controversie.
- **Il nome del tuo prodotto può essere registrato come marchio.** Non obbligatorio per il lancio, ma consideralo se il prodotto decolla. Deposito di marchio US: $250-350 per classe.
- **Le licenze open-source nelle tue dipendenze contano.** Se usi codice con licenza MIT, sei a posto. Se usi codice con licenza GPL in un prodotto commerciale, potresti dover rendere open-source il tuo prodotto. Controlla le licenze delle tue dipendenze prima di vendere.

```bash
# Controlla le licenze delle dipendenze del tuo progetto (Node.js)
npx license-checker --summary

# Controlla specificamente le licenze problematiche
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"

# Per progetti Rust
cargo install cargo-license
cargo license
```

**7. Assicurazione**

Non ti serve un'assicurazione per una libreria di componenti da $29. Ti serve un'assicurazione se:
- Fornisci servizi (consulenza, elaborazione dati) dove gli errori potrebbero causare perdite al cliente
- Il tuo prodotto gestisce dati sensibili (sanitari, finanziari)
- Stai firmando contratti con clienti enterprise (lo richiederanno)

Quando ne hai bisogno, l'assicurazione di responsabilità professionale (errori e omissioni / E&O) costa $500-1.500/anno per un business da sviluppatore in solitaria.

### Tocca a Te

1. **Leggi il tuo contratto di lavoro.** Se sei impiegato, trova la clausola sulla proprietà intellettuale e la clausola di non concorrenza. Categorizzale: Sicuro / Ambiguo / Restrittivo. Se Ambiguo o Restrittivo, consulta un avvocato del lavoro prima del lancio (molti offrono consulenze gratuite di 30 minuti).

2. **Genera i tuoi documenti legali.** Vai su Termly o Avodocs e genera un'informativa sulla privacy e dei termini di servizio per il tuo prodotto. Salvali come HTML o Markdown. Deployali su `/privacy` e `/terms` sul dominio del tuo prodotto.

3. **Prendi la tua decisione sull'entità.** In base alla guida sopra e alla tua residenza in {= regional.country | fallback("il tuo paese") =}, decidi: lanciare come ditta individuale (più veloce) o costituire prima una {= regional.business_entity_type | fallback("LLC/Ltd/equivalente") =} (più protezione). Scrivi la tua decisione e la tempistica.

4. **Controlla le tue dipendenze.** Esegui il license checker sul tuo progetto. Risolvi eventuali dipendenze GPL/AGPL prima di vendere un prodotto commerciale.

---

## Lezione 5: Canali di Distribuzione che Funzionano nel 2026

*"Costruirlo è il 20% del lavoro. Portarlo davanti alle persone è l'altro 80%."*

### La Realtà della Distribuzione

La maggior parte dei prodotti per sviluppatori fallisce non perché siano scadenti, ma perché nessuno sa che esistono. La distribuzione — portare il tuo prodotto davanti ai potenziali clienti — è la competenza in cui la maggior parte degli sviluppatori è più debole. Ed è la competenza che conta di più.

Ecco sette canali di distribuzione classificati per sforzo, tempistica e rendimento atteso. Non ti servono tutti e sette. Scegline 2-3 che corrispondono ai tuoi punti di forza e al tuo pubblico.

### Canale 1: Hacker News

**Sforzo:** Alto | **Tempistica:** Istantanea (0-48 ore) | **Natura:** Tutto-o-niente

Hacker News (https://news.ycombinator.com) è il canale di distribuzione a singolo evento con il più alto leverage per prodotti per sviluppatori. Un post Show HN in prima pagina può inviare 5.000-30.000 visitatori in 24 ore. Ma è imprevedibile — la maggior parte dei post ottiene zero trazione.

**Cosa funziona su HN:**
- Prodotti tecnici con dettagli di implementazione interessanti
- Tool focalizzati sulla privacy (il pubblico di HN tiene molto alla privacy)
- Tool open-source con un livello a pagamento
- Soluzioni innovative a problemi noti
- Prodotti con demo live

**Cosa non funziona su HN:**
- Lanci pesantemente marketing ("Rivoluzionaria piattaforma AI-powered...")
- Prodotti che sono wrapper di altri prodotti senza valore originale
- Qualsiasi cosa che sembra una pubblicità

**Il Playbook Show HN:**

```
PRIMA DI PUBBLICARE:
1. Studia i recenti Show HN di successo nella tua categoria
   https://hn.algolia.com — filtra per "Show HN", ordina per punteggio
2. Prepara il titolo del tuo post: "Show HN: [Nome] – [cosa fa, <70 caratteri]"
   Buono: "Show HN: ScrubLog – Strip PII from Log Files in One Command"
   Cattivo: "Show HN: Introducing ScrubLog, the AI-Powered Log Anonymization Platform"
3. Abbi una demo live pronta (i lettori di HN vogliono provare, non leggere)
4. Prepara risposte alle domande probabili (decisioni tecniche, ragionamento sul pricing)

PUBBLICAZIONE:
5. Pubblica tra le 7-9 AM orario US Eastern, da martedì a giovedì
   (traffico massimo, massima probabilità di trazione)
6. Il corpo del tuo post dovrebbe essere 4-6 paragrafi:
   - Cos'è (1 paragrafo)
   - Perché l'hai costruito (1 paragrafo)
   - Dettagli tecnici (1-2 paragrafi)
   - Cosa cerchi (feedback, domande specifiche)

DOPO LA PUBBLICAZIONE:
7. Resta online per 4 ore dopo la pubblicazione. Rispondi a OGNI commento.
8. Sii umile e tecnico. HN premia l'onestà sui limiti.
9. Se qualcuno trova un bug, correggilo in tempo reale e rispondi "Corretto, grazie."
10. Non chiedere ad amici di votare. HN ha il rilevamento dei circoli di voto.
```

**Risultati attesi (realistici):**
- 70% dei post Show HN: <10 punti, <500 visitatori
- 20% dei post Show HN: 10-50 punti, 500-3.000 visitatori
- 10% dei post Show HN: 50+ punti, 3.000-30.000 visitatori

È una lotteria con probabilità ponderate dallo sforzo. Un ottimo prodotto con un ottimo post ha forse il 30% di probabilità di trazione significativa. Non garantito. Ma il potenziale di rialzo è enorme.

### Canale 2: Reddit

**Sforzo:** Medio | **Tempistica:** 1-7 giorni | **Natura:** Sostenibile, ripetibile

Reddit è il canale di distribuzione più consistente per prodotti per sviluppatori. A differenza di HN (un colpo solo), Reddit ha centinaia di subreddit di nicchia dove il tuo prodotto è rilevante.

**Selezione dei subreddit:**

```
Subreddit generali per sviluppatori:
- r/SideProject (140K+ membri) — fatto per questo
- r/webdev (2.4M membri) — enorme, competitivo
- r/programming (6.3M membri) — molto competitivo, orientato alle news
- r/selfhosted (400K+ membri) — se il tuo prodotto è self-hostable

Specifici per framework/linguaggio:
- r/reactjs, r/nextjs, r/sveltejs, r/vuejs — per tool frontend
- r/rust, r/golang, r/python — per tool specifici per linguaggio
- r/node — per tool e pacchetti Node.js

Specifici per dominio:
- r/devops — per tool di infrastruttura/deployment
- r/machinelearning — per tool AI/ML
- r/datascience — per tool dati
- r/sysadmin — per tool di amministrazione/monitoraggio

La lunga coda:
- Cerca subreddit correlati alla tua nicchia specifica
- I subreddit più piccoli (10K-50K membri) spesso hanno tassi
  di conversione migliori di quelli enormi
```

**Regole di engagement su Reddit:**

1. **Abbi una storia Reddit reale** prima di pubblicare il tuo prodotto. Gli account che pubblicano solo autopromozione vengono segnalati e shadowbannati.
2. **Segui le regole di ogni subreddit** sull'autopromozione. La maggior parte la permette se sei un membro che contribuisce.
3. **Interagisci genuinamente.** Rispondi alle domande, fornisci valore, sii utile nei commenti di altri post. Poi condividi il tuo prodotto.
4. **Pubblica a orari diversi** per subreddit diversi. Controlla https://later.com/reddit o tool simili per gli orari di picco.

**Risultati attesi (realistici):**
- Post su r/SideProject: 20-100 upvote, 200-2.000 visitatori
- Subreddit di nicchia (50K membri): 10-50 upvote, 100-1.000 visitatori
- Prima pagina di r/webdev: 100-500 upvote, 2.000-10.000 visitatori

### Canale 3: Twitter/X

**Sforzo:** Medio | **Tempistica:** 2-4 settimane per costruire momentum | **Natura:** Si compone nel tempo

Twitter è un canale a costruzione lenta. Il tuo primo tweet di lancio otterrà 5 like dai tuoi amici. Ma se condividi costantemente il tuo processo di build, il tuo pubblico si compone.

**La Strategia Build-in-Public:**

```
Settimana 1: Inizia a condividere il tuo processo di build (prima del lancio)
- "Working on a [tipo di prodotto]. Here's the problem I'm solving: [screenshot]"
- "Day 3 of building [prodotto]. Got [feature] working: [GIF/screenshot]"

Settimana 2: Condividi intuizioni tecniche dal build
- "TIL you need to [lezione tecnica] when building [tipo di prodotto]"
- "Architecture decision: chose [X] over [Y] because [motivo]"

Settimana 3: Lancio
- Thread di lancio (formato dalla Lezione 1)
- Condividi metriche specifiche: "Day 1: X visitors, Y signups"

Settimana 4+: Continuativo
- Condividi feedback dei clienti (con permesso)
- Condividi milestone di entrate (alle persone piacciono i numeri reali)
- Condividi sfide e come le hai risolte
```

**Con chi interagire:**
- Segui e interagisci con sviluppatori nella tua nicchia
- Rispondi ai tweet di account più grandi con commenti ponderati (non autopromozione)
- Partecipa ai Twitter Spaces sul tuo argomento
- Quote-tweeta discussioni rilevanti con la tua prospettiva

**Risultati attesi (realistici):**
- 0-500 follower: I tweet di lancio ottengono 5-20 like, <100 visitatori
- 500-2.000 follower: I tweet di lancio ottengono 20-100 like, 100-500 visitatori
- 2.000-10.000 follower: I tweet di lancio ottengono 100-500 like, 500-5.000 visitatori

Twitter è un investimento di 6 mesi, non una strategia per il giorno del lancio. Inizia ora, anche prima che il tuo prodotto sia pronto.

### Canale 4: Product Hunt

**Sforzo:** Alto | **Tempistica:** 1 giorno di attività intensa | **Natura:** Boost una tantum

Product Hunt (https://producthunt.com) è una piattaforma di lancio dedicata. Un piazzamento nella top-5 giornaliera può inviare 3.000-15.000 visitatori. Ma richiede preparazione.

**Checklist per il Lancio su Product Hunt:**

```
2 SETTIMANE PRIMA:
- [ ] Crea un profilo maker su Product Hunt
- [ ] Costruisci la tua scheda PH: tagline, descrizione, immagini, video
- [ ] Prepara 4-5 screenshot/GIF di alta qualità
- [ ] Scrivi un "primo commento" che spieghi la tua motivazione
- [ ] Allinea 10-20 persone per supportare il giorno del lancio (non voti falsi —
      persone reali che proveranno il prodotto e lasceranno commenti genuini)
- [ ] Trova un "hunter" (qualcuno con un grande seguito PH per inviare il tuo prodotto)
      o invialo tu stesso

GIORNO DEL LANCIO (00:01 AM Pacific Time):
- [ ] Sii online dalla mezzanotte PT. PH si resetta a mezzanotte.
- [ ] Pubblica il tuo "primo commento" immediatamente
- [ ] Condividi il link PH su Twitter, LinkedIn, email, Discord
- [ ] Rispondi a OGNI commento sulla tua scheda PH
- [ ] Pubblica aggiornamenti durante tutto il giorno ("Appena rilasciata una fix per [X]!")
- [ ] Monitora tutto il giorno fino a mezzanotte PT

DOPO:
- [ ] Ringrazia tutti quelli che hanno supportato
- [ ] Scrivi un post "lezioni apprese" (buon contenuto per Twitter/blog)
- [ ] Inserisci il badge PH sulla tua landing page (prova sociale)
```

> **Errore Comune:** Lanciare su Product Hunt prima che il prodotto sia pronto. PH ti dà un colpo solo. Una volta che lanci un prodotto, non puoi rilanciarlo. Aspetta finché il tuo prodotto è rifinito, la tua landing page converte e il flusso di pagamento funziona. PH dovrebbe essere il tuo "grande lancio" — non il tuo lancio soft.

**Risultati attesi (realistici):**
- Top 5 giornaliera: 3.000-15.000 visitatori, 50-200 upvote
- Top 10 giornaliera: 1.000-5.000 visitatori, 20-50 upvote
- Sotto la top 10: <1.000 visitatori. Impatto duraturo minimo.

### Canale 5: Dev.to / Hashnode / Post Tecnici sul Blog

**Sforzo:** Basso-medio | **Tempistica:** Risultati SEO in 1-3 mesi | **Natura:** Lunga coda, si compone per sempre

Scrivi post tecnici sul blog che risolvono problemi correlati al tuo prodotto, e menziona il tuo prodotto come soluzione.

**Strategia di contenuto:**

```
Per ogni prodotto, scrivi 3-5 post sul blog:

1. "Come [risolvere il problema che il tuo prodotto risolve] nel 2026"
   - Insegna l'approccio manuale, poi menziona il tuo prodotto come scorciatoia

2. "Ho costruito [prodotto] in 48 ore — ecco cosa ho imparato"
   - Contenuto build-in-public. Dettagli tecnici + riflessione onesta.

3. "[Competitor] vs [Il Tuo Prodotto]: Confronto Onesto"
   - Sii genuinamente equo. Menziona dove il competitor vince.
   - Questo cattura il traffico di ricerca del confronto-shopping.

4. "[Concetto tecnico correlato al tuo prodotto] spiegato"
   - Pura educazione. Menziona il tuo prodotto una volta alla fine.

5. "I tool che uso per [il dominio del tuo prodotto] nel 2026"
   - Formato listicle. Includi il tuo prodotto insieme agli altri.
```

**Dove pubblicare:**
- **Dev.to** (https://dev.to) — Grande pubblico di sviluppatori, buon SEO, gratuito
- **Hashnode** (https://hashnode.com) — Buon SEO, opzione dominio personalizzato, gratuito
- **Il tuo blog** — Migliore per il SEO a lungo termine, possiedi il contenuto
- **Cross-posta ovunque.** Scrivi una volta, pubblica su tutti e tre. Usa URL canonici per evitare penalizzazioni SEO.

**Risultati attesi per post:**
- Giorno 1: 100-1.000 visualizzazioni (distribuzione della piattaforma)
- Mese 1-3: 50-200 visualizzazioni/mese (traffico di ricerca in costruzione)
- Mese 6+: 100-500 visualizzazioni/mese (traffico di ricerca che si compone)

Un singolo post sul blog ben scritto può portare 200+ visitatori al mese per anni. Cinque post portano 1.000+/mese. Questo si compone.

### Canale 6: Outreach Diretto

**Sforzo:** Alto | **Tempistica:** Immediata | **Natura:** Tasso di conversione più alto

Email a freddo e DM hanno il tasso di conversione più alto di qualsiasi canale — ma anche il più alto sforzo per lead. Usa questo per prodotti a prezzo più alto ($99+) o vendite B2B.

**Template email per raggiungere potenziali clienti:**

```
Oggetto: Domanda veloce su [il loro pain point specifico]

Ciao [nome],

Ho visto il tuo [tweet/post/commento] su [problema specifico che hanno menzionato].

Ho costruito [nome prodotto] specificamente per questo — [descrizione
in una frase di cosa fa].

Saresti disponibile a provarlo? Felice di darti accesso gratuito
per un feedback.

[Il tuo nome]
[Link al prodotto]
```

**Regole per l'outreach a freddo:**
- Contatta solo persone che hanno espresso pubblicamente il problema che il tuo prodotto risolve
- Fai riferimento al loro post/commento specifico (dimostra che non stai inviando email di massa)
- Offri valore (accesso gratuito, sconto) piuttosto che chiedere soldi immediatamente
- Mantienilo sotto 5 frasi
- Invia da un indirizzo email reale (tu@tuodominio.com, non gmail)
- Fai follow-up una volta dopo 3-4 giorni. Se nessuna risposta, fermati.

**Risultati attesi:**
- Tasso di risposta: 10-20% (email a freddo a destinatari rilevanti)
- Conversione da risposta a prova: 30-50%
- Conversione da prova a pagamento: 20-40%
- Conversione effettiva: 1-4% delle persone contattate diventano clienti

Per un prodotto da $99, inviare email a 100 persone = 1-4 vendite = $99-396. Non scalabile, ma eccellente per ottenere clienti precoci e feedback.

### Canale 7: SEO

**Sforzo:** Basso continuativo | **Tempistica:** 3-6 mesi per risultati | **Natura:** Si compone per sempre

Il SEO è il miglior canale di distribuzione a lungo termine. È lento a partire ma una volta che funziona, invia traffico gratuito indefinitamente.

**Strategia SEO focalizzata sugli sviluppatori:**

```
1. Punta le keyword long-tail (più facili da posizionare):
   Invece di: "dashboard components"
   Punta: "tailwind dashboard components react typescript"

2. Crea una pagina per keyword:
   Ogni post sul blog o pagina docs punta a una specifica query di ricerca

3. Implementazione tecnica:
   - Usa la generazione di siti statici (Astro, Next.js SSG) per caricamenti veloci
   - Aggiungi meta description a ogni pagina
   - Usa HTML semantico (gerarchia h1, h2, h3)
   - Aggiungi alt text a ogni immagine
   - Invia la sitemap a Google Search Console

4. Contenuto che si posiziona per tool per sviluppatori:
   - Pagine di documentazione (sorprendentemente buone per il SEO)
   - Pagine di confronto ("X vs Y")
   - Pagine tutorial ("Come fare X con Y")
   - Pagine changelog (segnale di contenuto fresco per Google)
```

```bash
# Invia la tua sitemap a Google Search Console
# 1. Vai su https://search.google.com/search-console
# 2. Aggiungi la tua proprietà (dominio o prefisso URL)
# 3. Verifica la proprietà (record DNS TXT o file HTML)
# 4. Invia l'URL della tua sitemap: tuodominio.com/sitemap.xml

# Se usi Astro:
pnpm add @astrojs/sitemap
# La sitemap viene auto-generata su /sitemap.xml

# Se usi Next.js, aggiungi a next-sitemap.config.js:
# pnpm add next-sitemap
```

**Risultati attesi:**
- Mese 1-3: Traffico organico minimo (<100/mese)
- Mese 3-6: Traffico in crescita (100-500/mese)
- Mese 6-12: Traffico significativo (500-5.000/mese)
- Mese 12+: Traffico che si compone e cresce senza sforzo

{@ temporal market_timing @}

### Framework per la Selezione dei Canali

Non puoi farli tutti e sette bene. Scegline 2-3 in base a questa matrice:

| Se stai... | Prioritizza | Salta |
|---|---|---|
| Lanciando questo weekend | Reddit + HN | SEO, Twitter (troppo lenti) |
| Costruendo un pubblico prima | Twitter + Post sul blog | Outreach diretto, PH |
| Vendendo un prodotto da $99+ | Outreach diretto + HN | Dev.to (il pubblico si aspetta gratuito) |
| Giocando sul lungo periodo | SEO + Post sul blog + Twitter | PH (un colpo solo, usalo dopo) |
| Non anglofono | Dev.to + Reddit (globali) | HN (centrato sugli USA) |

### Tocca a Te

1. **Scegli i tuoi 2-3 canali.** In base alla matrice sopra e al tipo di prodotto, scegli i canali su cui ti concentrerai. Scrivili con la tempistica pianificata per ciascuno.

2. **Scrivi il tuo post Reddit.** Usando il template dalla Lezione 1, scrivi la bozza del tuo post su r/SideProject adesso. Salvala. La pubblicherai il giorno del lancio.

3. **Scrivi il tuo primo post sul blog.** Scrivi una bozza di un post "Come [risolvere il problema che il tuo prodotto risolve]". Questo va su Dev.to o sul tuo blog entro la prima settimana dal lancio. Punta a 1.500-2.000 parole.

4. **Configura Google Search Console.** Ci vogliono 5 minuti e ti dà dati SEO dal primo giorno. Fallo prima del lancio così hai dati di riferimento.

---

## Lezione 6: La Tua Checklist di Lancio

*"La speranza non è una strategia di lancio. Le checklist sì."*

### La Checklist Pre-Lancio

Passa in rassegna ogni elemento. Non lanciare finché ogni elemento "Obbligatorio" non è spuntato. Gli elementi "Raccomandati" possono essere fatti nella Settimana 1 se necessario.

**Prodotto (Obbligatorio):**

```
- [ ] La feature core funziona come descritto sulla landing page
- [ ] Nessun bug critico nel flusso acquisto → consegna
- [ ] Funziona in Chrome, Firefox e Safari (per prodotti web)
- [ ] Landing page responsive per mobile (50%+ del traffico è mobile)
- [ ] I messaggi di errore sono utili, non stack trace
- [ ] Stati di caricamento per qualsiasi operazione asincrona
```

**Landing Page (Obbligatorio):**

```
- [ ] Titolo chiaro: cosa fa in 8 parole o meno
- [ ] Dichiarazione del problema: 3 pain point nel linguaggio del cliente
- [ ] Sezione soluzione: screenshot o demo del prodotto
- [ ] Pricing: visibile, chiaro, con pulsante di acquisto
- [ ] Call to action: un pulsante primario, visibile above the fold
- [ ] Link all'informativa sulla privacy nel footer
- [ ] Link ai termini di servizio nel footer
```

**Pagamenti (Obbligatorio):**

```
- [ ] Flusso di checkout testato end-to-end in modalità test
- [ ] Flusso di checkout testato end-to-end in modalità live (acquisto di prova da $1)
- [ ] Il webhook riceve la conferma di pagamento
- [ ] Il cliente riceve l'accesso al prodotto dopo il pagamento
- [ ] Processo di rimborso documentato (RICEVERAI richieste di rimborso)
- [ ] Ricevuta/fattura inviata automaticamente
```

**Infrastruttura (Obbligatorio):**

```
- [ ] Dominio personalizzato che punta al sito live
- [ ] HTTPS funzionante (lucchetto verde)
- [ ] Monitoraggio uptime attivo
- [ ] Script analytics installato e che riceve dati
- [ ] Email di contatto funzionante (tu@tuodominio.com)
```

**Distribuzione (Obbligatorio):**

```
- [ ] Post Reddit preparato e pronto
- [ ] Post Show HN preparato e pronto (se applicabile)
- [ ] Thread di lancio Twitter preparato
- [ ] 2-3 community identificate per la condivisione
```

**Raccomandato (Settimana 1):**

```
- [ ] Meta tag OpenGraph per anteprime di condivisione social
- [ ] Pagina 404 personalizzata
- [ ] Pagina o sezione FAQ
- [ ] Sequenza email di onboarding clienti (benvenuto + primi passi)
- [ ] Pagina changelog (anche se vuota — mostra impegno negli aggiornamenti)
- [ ] Post sul blog: "Ho costruito [prodotto] in 48 ore"
- [ ] Google Search Console verificato e sitemap inviata
```

### Azioni Post-Lancio

**Giorno 1 (Giorno del Lancio):**

```
Mattina:
- [ ] Pubblica su Reddit (r/SideProject + 1 subreddit di nicchia)
- [ ] Pubblica Show HN (se applicabile)
- [ ] Pubblica thread di lancio su Twitter

Tutto il giorno:
- [ ] Rispondi a OGNI commento su Reddit, HN e Twitter
- [ ] Monitora log degli errori e analytics in tempo reale
- [ ] Correggi immediatamente i bug scoperti dagli utenti
- [ ] Invia email di ringraziamento personale a ogni cliente

Sera:
- [ ] Controlla le metriche: visitatori, tasso di conversione, entrate
- [ ] Fai uno screenshot del dashboard analytics (lo vorrai dopo)
- [ ] Scrivi i 3 pezzi di feedback più comuni
```

**Settimana 1:**

```
- [ ] Rispondi a tutti i feedback e richieste di supporto entro 24 ore
- [ ] Correggi i top 3 bug/problemi identificati durante il lancio
- [ ] Scrivi e pubblica il tuo primo post sul blog
- [ ] Invia una email di follow-up a tutti i clienti chiedendo feedback
- [ ] Rivedi l'analytics: quali pagine hanno i bounce rate più alti?
- [ ] Configura un metodo semplice di raccolta feedback (email, Typeform o Canny)

Metriche settimanali da registrare:
| Metrica             | Obiettivo | Effettivo |
|---------------------|-----------|-----------|
| Visitatori unici    | 500+      |           |
| Tasso click checkout| 2-5%      |           |
| Conversione acquisto| 1-3%      |           |
| Entrate             | $50+      |           |
| Richieste supporto  | <10       |           |
| Richieste rimborso  | <2        |           |
```

**Mese 1:**

```
- [ ] Spedisci 4 miglioramenti settimanali basati sul feedback dei clienti
- [ ] Pubblica 2+ post sul blog (costruzione SEO)
- [ ] Raccogli 3+ testimonianze dai clienti
- [ ] Aggiungi le testimonianze alla landing page
- [ ] Valuta il pricing: troppo alto? troppo basso? (rivedi i dati di conversione)
- [ ] Pianifica il tuo "grande lancio" su Product Hunt (se applicabile)
- [ ] Inizia a costruire una mailing list per lanci di prodotti futuri
- [ ] Rivedi e aggiusta la tua strategia sui canali di distribuzione

Revisione finanziaria mensile:
| Categoria              | Importo   |
|------------------------|-----------|
| Entrate lorde          | $         |
| Commissioni processore | $         |
| Costi hosting/infra    | $         |
| Costi API              | $         |
| Profitto netto         | $         |
| Ore investite          |           |
| Tariffa oraria effettiva | $      |
```

### Il Dashboard delle Metriche

Configura un semplice dashboard delle metriche che controlli quotidianamente. Non deve essere sofisticato — un foglio di calcolo funziona.

```
=== METRICHE GIORNALIERE (controlla ogni mattina) ===

Data: ___
Visitatori ieri: ___
Nuovi clienti ieri: ___
Entrate ieri: $___
Richieste di supporto: ___
Uptime: ___%

=== METRICHE SETTIMANALI (controlla ogni lunedì) ===

Settimana del: ___
Visitatori totali: ___
Clienti totali: ___
Entrate totali: $___
Tasso di conversione: ___% (clienti / visitatori)
Pagina più visitata: ___
Sorgente di traffico principale: ___
Tema di feedback principale: ___

=== METRICHE MENSILI (controlla il 1° del mese) ===

Mese: ___
Entrate totali: $___
Spese totali: $___
Profitto netto: $___
Clienti totali: ___
Rimborsi: ___
Tasso di churn (abbonamenti): ___%
MRR (Monthly Recurring Revenue): $___
Tasso di crescita vs. mese scorso: ___%
```

**Configurazione analytics rispettosa della privacy:**

```javascript
// Se usi Plausible, ottieni la maggior parte di questo nel loro dashboard.
// Per il tracking di eventi personalizzati:

// Traccia i click sul checkout
document.querySelector('#buy-button').addEventListener('click', () => {
  plausible('Checkout Click', {
    props: { tier: 'pro', price: '59' }
  });
});

// Traccia gli acquisti riusciti (chiamalo dal tuo handler di successo webhook)
plausible('Purchase', {
  props: { tier: 'pro', revenue: '59' }
});
```

### Quando Raddoppiare, Pivotare o Chiudere

Dopo 30 giorni di dati, hai abbastanza segnale per prendere una decisione:

**Raddoppia (continua, investi di più):**

```
Segnali:
- Le entrate crescono settimana dopo settimana (anche se lentamente)
- I clienti forniscono richieste di feature specifiche (vogliono DI PIU')
- Il tasso di conversione è stabile o in miglioramento
- Stai ottenendo traffico organico (persone che ti trovano senza i tuoi post)
- Almeno un cliente ha detto "questo mi ha fatto risparmiare [tempo/soldi]"

Azioni:
- Aumenta gli sforzi di distribuzione (aggiungi un canale)
- Spedisci la feature più richiesta
- Alza leggermente i prezzi
- Inizia a costruire una mailing list per lanci futuri
```

**Pivota (cambia angolo, mantieni il core):**

```
Segnali:
- Visitatori ma nessuna vendita (le persone sono interessate ma non comprano)
- Vendite da un pubblico inaspettato (persone diverse da quelle che puntavi)
- I clienti usano il prodotto diversamente da come ti aspettavi
- Il feedback punta costantemente a un problema diverso da quello che stai risolvendo

Azioni:
- Riscrivi la landing page per il pubblico/caso d'uso reale
- Aggiusta il pricing in base alla disponibilità a pagare del pubblico reale
- Riordina le priorità delle feature verso ciò che le persone usano davvero
- Mantieni il codice, cambia il posizionamento
```

**Chiudi (fermati, impara, costruisci qualcos'altro):**

```
Segnali:
- Nessun visitatore nonostante gli sforzi di distribuzione (problema di domanda)
- Visitatori ma zero click sul checkout (problema di posizionamento/pricing
  che persiste dopo gli aggiustamenti)
- Entrate stagnanti per 4+ settimane senza trend di crescita
- Temi lavorarci (la motivazione conta per prodotti in solitaria)
- Il mercato è cambiato (competitor lanciato, tecnologia cambiata)

Azioni:
- Scrivi un post-mortem: cosa ha funzionato, cosa no, cosa hai imparato
- Salva il codice — pezzi potrebbero essere utili nel tuo prossimo prodotto
- Prenditi una settimana di pausa dal costruire
- Inizia il processo di validazione per una nuova idea
- Questo non è un fallimento. Sono dati. La maggior parte dei prodotti non funziona.
  Gli sviluppatori che fanno soldi sono quelli che spediscono 5 prodotti,
  non quelli che passano un anno su uno.
```

### Il Template del Documento di Lancio

Questo è il tuo deliverable per il Modulo E. Crea questo documento e compilalo man mano che esegui il tuo lancio.

```markdown
# Documento di Lancio: [Nome Prodotto]

## Pre-Lancio

### Riepilogo Validazione
- **Volume di ricerca:** [numeri da Google Trends/Ahrefs]
- **Prove dai thread:** [link a 5+ thread che mostrano domanda]
- **Audit competitor:** [3+ competitor con punti di forza/debolezza]
- **Prove "10 persone pagherebbero":** [come hai validato questo]

### Prodotto
- **URL:** [URL del prodotto live]
- **Dominio:** [dominio acquistato]
- **Hosting:** [piattaforma]
- **Feature core (v0.1):**
  1. [Feature 1]
  2. [Feature 2]
  3. [Feature 3]

### Pricing
- **Prezzo:** $[importo]
- **Struttura livelli:** [Base/Pro/Team o livello singolo]
- **Piattaforma di pagamento:** [Lemon Squeezy/Stripe]
- **URL checkout:** [link]

### Legale
- **Informativa sulla privacy:** [URL]
- **Termini di servizio:** [URL]
- **Entità commerciale:** [tipo o "ditta individuale"]

## Lancio

### Canali di Distribuzione
| Canale  | URL del Post | Data Pubblicazione | Risultati |
|---------|-------------|-------------------|-----------|
| Reddit  | [link]      | [data]            | [visitatori, upvote] |
| HN      | [link]      | [data]            | [visitatori, punti] |
| Twitter | [link]      | [data]            | [impressioni, click] |

### Metriche Giorno 1
- Visitatori: ___
- Click checkout: ___
- Acquisti: ___
- Entrate: $___

### Metriche Settimana 1
- Visitatori totali: ___
- Acquisti totali: ___
- Entrate totali: $___
- Tasso di conversione: ___%
- Top feedback: ___

### Metriche Mese 1
- Entrate totali: $___
- Spese totali: $___
- Profitto netto: $___
- Clienti totali: ___
- Decisione: [ ] Raddoppiare [ ] Pivotare [ ] Chiudere

## Roadmap Post-Lancio
- Settimana 2: [miglioramento pianificato]
- Settimana 3: [miglioramento pianificato]
- Settimana 4: [miglioramento pianificato]
- Mese 2: [feature/espansione pianificata]

## Lezioni Apprese
- Cosa ha funzionato: ___
- Cosa non ha funzionato: ___
- Cosa farei diversamente: ___
```

### Integrazione 4DA

> **Integrazione 4DA:** I segnali azionabili di 4DA classificano i contenuti per urgenza. Un segnale "critico" su una vulnerabilità in un pacchetto popolare significa: costruisci il fix o il tool di migrazione ADESSO, prima di chiunque altro. Un segnale "trend in crescita" su un nuovo framework significa: costruisci lo starter kit questo weekend mentre la competizione è quasi zero. Lo sprint di 48 ore della Lezione 1 funziona meglio quando la tua idea viene da un segnale sensibile al tempo. Connetti il tuo feed di intelligence 4DA al tuo calendario di sprint — quando appare un'opportunità ad alta urgenza, blocca il prossimo weekend ed esegui. La differenza tra gli sviluppatori che colgono le opportunità e quelli che le mancano non è il talento. È la velocità. 4DA ti dà il radar. Questo modulo ti dà la sequenza di lancio. Insieme, trasformano segnali in entrate.

### Tocca a Te

1. **Completa la checklist pre-lancio.** Passa in rassegna ogni elemento. Segna ognuno come fatto o pianifica quando lo farai. Non saltare gli elementi "Obbligatori".

2. **Crea il tuo Documento di Lancio.** Copia il template sopra nel tuo tool di documenti preferito. Compila tutto ciò che sai adesso. Lascia spazi vuoti per le metriche che compilerai durante e dopo il lancio.

3. **Fissa la tua data di lancio.** Apri il tuo calendario. Scegli un sabato specifico nelle prossime 2 settimane. Scrivilo. Dillo a qualcuno — un amico, un partner, un follower su Twitter. La responsabilità lo rende reale.

4. **Fissa i tuoi criteri di chiusura.** Prima del lancio, decidi: "Se ho meno di [X] vendite dopo 30 giorni nonostante [Y] sforzo di distribuzione, [pivoterò/chiuderò]." Scrivi questo nel tuo Documento di Lancio. Avere criteri pre-impegnati ti impedisce di investire mesi in un prodotto morto a causa del bias del costo sommerso.
{? if progress.completed("S") ?}
   Fai riferimento al tuo Sovereign Stack Document dal Modulo S — i tuoi vincoli di budget e costi operativi definiscono cosa significa "profittevole" per la tua situazione specifica.
{? endif ?}

5. **Spediscilo.** Hai il playbook. Hai i tool. Hai la conoscenza. L'unica cosa che resta è l'azione. Internet ti aspetta.

---

## Modulo E: Completato

### Cosa Hai Costruito in Due Settimane

{? if dna.identity_summary ?}
> **La tua identità da sviluppatore:** {= dna.identity_summary | fallback("Non ancora profilata") =}. Tutto ciò che hai costruito in questo modulo sfrutta questa identità — la tua velocità di spedizione è funzione della tua competenza esistente.
{? endif ?}

Guarda cosa hai adesso che non avevi quando hai iniziato questo modulo:

1. **Un framework di esecuzione in 48 ore** che puoi ripetere per ogni prodotto che costruisci — da idea validata a prodotto live in un weekend.
2. **Una mentalità di spedizione** che dà priorità all'esistenza sulla perfezione, ai dati sulle congetture e all'iterazione sulla pianificazione.
3. **Una strategia di pricing** fondata sulla psicologia reale e i numeri reali, non sulla speranza e il sottoprezzare.
4. **Una base legale** che ti protegge senza paralizzarti — informativa sulla privacy, termini, piano per l'entità.
5. **Un playbook di distribuzione** con template specifici, tempistiche e risultati attesi per sette canali.
6. **Una checklist di lancio e un sistema di tracking** che trasformano il caos in processo — ripetibile, misurabile, migliorabile.
7. **Un prodotto live, che accetta pagamenti, con esseri umani reali che lo visitano.**

Quest'ultimo è quello che conta. Tutto il resto è preparazione. Il prodotto è la prova.

### Cosa Viene Dopo: Modulo E2 — Vantaggio Evolutivo

Il Modulo E1 ti ha portato al lancio. Il Modulo E2 ti mantiene in vantaggio.

Ecco cosa copre il Modulo E2:

- **Sistemi di rilevamento dei trend** — come individuare opportunità 2-4 settimane prima che diventino ovvie
- **Monitoraggio competitivo** — tracciare cosa altri nel tuo spazio stanno costruendo e prezzando
- **Cavalcare le onde tecnologiche** — quando adottare nuove tecnologie nei tuoi prodotti e quando aspettare
- **Customer development** — trasformare i tuoi primi 10 clienti nel tuo comitato consultivo di prodotto
- **La decisione del secondo prodotto** — quando costruire il prodotto #2 vs. migliorare il prodotto #1

Gli sviluppatori che generano entrate consistenti non sono quelli che lanciano una volta. Sono quelli che lanciano, iterano e restano davanti al mercato. Il Modulo E2 ti dà il sistema per restare in vantaggio.

### La Roadmap Completa di STREETS

| Modulo | Titolo | Focus | Durata |
|--------|--------|-------|--------|
| **S** | Sovereign Setup | Infrastruttura, legale, budget | Settimane 1-2 |
| **T** | Technical Moats | Vantaggi difendibili, asset proprietari | Settimane 3-4 |
| **R** | Revenue Engines | Playbook di monetizzazione specifici con codice | Settimane 5-8 |
| **E** | Execution Playbook | Sequenze di lancio, pricing, primi clienti | Settimane 9-10 (completato) |
| **E** | Evolving Edge | Restare in vantaggio, rilevamento trend, adattamento | Settimane 11-12 |
| **T** | Tactical Automation | Automatizzare le operazioni per reddito passivo | Settimane 13-14 |
| **S** | Stacking Streams | Sorgenti di reddito multiple, strategia di portafoglio | Settimane 15-16 |

Sei oltre il punto di metà. Hai un prodotto live. Questo ti mette davanti al 95% degli sviluppatori che vogliono costruire reddito indipendente ma non arrivano mai fin qui.

> **Progresso STREETS:** {= progress.completed_count | fallback("0") =} di {= progress.total_count | fallback("7") =} moduli completati. {? if progress.completed_modules ?}Completati: {= progress.completed_modules | fallback("Nessuno ancora") =}.{? endif ?}

Adesso fallo crescere.

---

**Il tuo prodotto è live. Il tuo checkout funziona. Gli esseri umani possono pagarti.**

**Tutto ciò che viene dopo è ottimizzazione. E l'ottimizzazione è la parte divertente.**

*Il tuo setup. Le tue regole. Le tue entrate.*
