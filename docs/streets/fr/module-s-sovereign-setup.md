# Module S : Configuration Souveraine

**Cours STREETS sur les Revenus pour Developpeurs — Module Gratuit**
*Semaines 1-2 | 6 Lecons | Livrable : Ton Document de Stack Souveraine*

> "Ta machine est ton infrastructure commerciale. Configure-la comme telle."

---

Tu possedes deja l'outil de generation de revenus le plus puissant que la plupart des gens n'auront jamais : un poste de travail developpeur avec une connexion internet, de la puissance de calcul locale, et les competences pour tout assembler.

La plupart des developpeurs traitent leur machine comme un produit grand public. Quelque chose sur lequel ils jouent, codent, naviguent. Mais cette meme machine — celle qui est posee sous ton bureau en ce moment — peut faire de l'inference, servir des API, traiter des donnees et generer des revenus 24 heures sur 24 pendant que tu dors.

Ce module consiste a regarder ce que tu possedes deja sous un angle different. Non pas "que puis-je construire ?" mais "que puis-je vendre ?"

A la fin de ces deux semaines, tu auras :

- Un inventaire clair de tes capacites generatrices de revenus
- Un stack LLM local de qualite production
- Une base juridique et financiere (meme minimale)
- Un Document de Stack Souveraine ecrit qui deviendra le plan de ton activite

Pas de bla-bla. Pas de "crois en toi." Des vrais chiffres, des vraies commandes, des vraies decisions.

{@ mirror sovereign_readiness @}

C'est parti.

---

## Lecon 1 : L'Audit de la Machine

*"Tu n'as pas besoin d'une 4090. Voici ce qui compte vraiment."*

### Ta Machine Est un Actif Commercial

Quand une entreprise evalue son infrastructure, elle ne se contente pas de lister des specs — elle fait correspondre les capacites aux opportunites de revenus. C'est exactement ce que tu vas faire maintenant.

{? if computed.profile_completeness != "0" ?}
> **Ta Machine Actuelle :** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} coeurs / {= profile.cpu.threads | fallback("?") =} threads), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} libre / {= profile.storage.total | fallback("?") =} total ({= profile.storage.type | fallback("unknown") =}), sous {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =}.
{? endif ?}

Ouvre un terminal et lance les commandes suivantes. Note chaque chiffre. Tu en auras besoin pour ton Document de Stack Souveraine dans la Lecon 6.

### Inventaire Materiel

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

**Ce qui compte pour les revenus :**
- Le nombre de coeurs determine combien de taches simultanees ta machine peut gerer. Faire tourner un LLM local tout en traitant un job batch en parallele necessite un vrai parallelisme.
{? if profile.cpu.cores ?}
- *Ton {= profile.cpu.model | fallback("CPU") =} a {= profile.cpu.cores | fallback("?") =} coeurs — consulte le tableau des exigences ci-dessous pour voir quels moteurs de revenus ton CPU supporte.*
{? endif ?}
- Pour la plupart des moteurs de revenus de ce cours, n'importe quel CPU moderne de 8+ coeurs des 5 dernieres annees suffit.
- Si tu fais tourner des LLM locaux uniquement sur CPU (pas de GPU), vise 16+ coeurs. Un Ryzen 7 5800X ou un Intel i7-12700 est le minimum pratique.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**Ce qui compte pour les revenus :**
- 16 Go : Le strict minimum. Tu peux faire tourner des modeles 7B et faire du travail d'automatisation de base.
- 32 Go : Confortable. Fais tourner des modeles 13B localement, gere plusieurs projets, garde ton environnement de dev actif en parallele de tes charges de travail generatrices de revenus.
- 64 Go+ : Tu peux faire tourner des modeles 30B+ sur CPU, ou garder plusieurs modeles charges en memoire. C'est la que les choses deviennent interessantes pour vendre des services d'inference.
{? if profile.ram.total ?}
*Ton systeme a {= profile.ram.total | fallback("?") =} de RAM. Consulte le tableau ci-dessus pour voir dans quelle categorie tu te situes — ca affecte directement quels modeles locaux sont pratiques pour tes charges de travail generatrices de revenus.*
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

**Ce qui compte pour les revenus :**

C'est la spec sur laquelle les gens sont obsedes, et voici la verite honnete : **ton GPU determine ta categorie de LLM local, et ta categorie de LLM local determine quels flux de revenus tournent le plus vite.** Mais ca ne determine pas si tu peux gagner de l'argent ou non.

| VRAM | Capacite LLM | Pertinence pour les Revenus |
|------|---------------|------------------|
| 0 (CPU uniquement) | Modeles 7B a ~5 tokens/sec | Traitement batch, travail asynchrone. Lent mais fonctionnel. |
| 6-8 Go (RTX 3060, etc.) | Modeles 7B a ~30 tok/sec, 13B quantifie | Suffisant pour la plupart des flux de revenus d'automatisation. |
| 12 Go (RTX 3060 12GB, 4070) | 13B a pleine vitesse, 30B quantifie | Le point ideal. La plupart des moteurs de revenus tournent bien ici. |
| 16-24 Go (RTX 4090, 3090) | Modeles 30B-70B | Categorie premium. Vends une qualite que les autres ne peuvent pas atteindre localement. |
| 48 Go+ (double GPU, A6000) | 70B+ a bonne vitesse | Inference locale de niveau entreprise. Avantage concurrentiel serieux. |
| Apple Silicon 32Go+ (M2/M3 Pro/Max) | 30B+ avec memoire unifiee | Excellente efficacite. Cout energetique inferieur a l'equivalent NVIDIA. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **Ton GPU :** {= profile.gpu.model | fallback("Unknown") =} avec {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}tu es dans la categorie premium. Les modeles 30B-70B sont a ta portee localement. C'est un avantage concurrentiel serieux.{? elif computed.gpu_tier == "sweet_spot" ?}tu es au point ideal. 13B a pleine vitesse, 30B quantifie. La plupart des moteurs de revenus tournent bien ici.{? elif computed.gpu_tier == "capable" ?}tu peux faire tourner des modeles 7B a bonne vitesse et des 13B quantifies. Suffisant pour la plupart des flux de revenus d'automatisation.{? else ?}tu as de l'acceleration GPU disponible. Consulte le tableau ci-dessus pour voir ou tu te situes.{? endif ?}
{? else ?}
> **Pas de GPU dedie detecte.** Tu feras de l'inference sur CPU, ce qui signifie ~5-12 tokens/sec sur des modeles 7B. C'est suffisant pour le traitement batch et le travail asynchrone. Utilise des appels API pour combler le manque de vitesse sur les sorties destinees aux clients.
{? endif ?}

> **Parlons franchement :** Si tu as une RTX 3060 12GB, tu es mieux positionne que 95% des developpeurs qui essaient de monetiser l'IA. Arrete d'attendre une 4090. La 3060 12GB est la Honda Civic de l'IA locale — fiable, efficace, elle fait le boulot. L'argent que tu depenserais pour une mise a niveau GPU est mieux investi en credits API pour la qualite destinee aux clients, pendant que tes modeles locaux gerent le gros du travail.

#### Stockage

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**Ce qui compte pour les revenus :**
- Les modeles LLM prennent de la place : modele 7B = ~4 Go, 13B = ~8 Go, 70B = ~40 Go (quantifie).
- Tu as besoin d'espace pour les donnees de projet, les bases de donnees, les caches et les artefacts de sortie.
- Un SSD est non-negociable pour tout ce qui est destine aux clients. Le chargement de modeles depuis un HDD ajoute 30 a 60 secondes de temps de demarrage.
- Minimum pratique : 500 Go SSD avec au moins 100 Go de libre.
- Confortable : 1 To SSD. Garde les modeles sur le SSD, archive sur HDD.
{? if profile.storage.free ?}
*Tu as {= profile.storage.free | fallback("?") =} de libre sur {= profile.storage.type | fallback("your drive") =}. {? if profile.storage.type == "SSD" ?}Bien — un SSD signifie un chargement rapide des modeles.{? elif profile.storage.type == "NVMe" ?}Excellent — NVMe est l'option la plus rapide pour le chargement de modeles.{? else ?}Envisage un SSD si tu n'en as pas deja un — ca fait une vraie difference pour les temps de chargement des modeles.{? endif ?}*
{? endif ?}

#### Reseau

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**Ce qui compte pour les revenus :**
{? if profile.network.download ?}
*Ta connexion : {= profile.network.download | fallback("?") =} descendant / {= profile.network.upload | fallback("?") =} montant.*
{? endif ?}
- Vitesse descendante : 50+ Mbps. Necessaire pour telecharger des modeles, des paquets et des donnees.
- Vitesse montante : C'est le goulot d'etranglement que la plupart des gens ignorent. Si tu sers quoi que ce soit (API, resultats traites, livrables), le debit montant compte.
  - 10 Mbps : Adequat pour la livraison asynchrone (fichiers traites, resultats batch).
  - 50+ Mbps : Requis si tu fais tourner un endpoint API local que des services externes appellent.
  - 100+ Mbps : Confortable pour tout ce qui est couvert dans ce cours.
- Latence : Moins de 50ms vers les principaux fournisseurs cloud. Lance `ping api.openai.com` et `ping api.anthropic.com` pour verifier.

#### Disponibilite

C'est la spec a laquelle personne ne pense, mais elle separe les amateurs des gens qui gagnent de l'argent pendant qu'ils dorment.

Demande-toi :
- Ta machine peut-elle tourner 24h/24, 7j/7 ? (Alimentation, refroidissement, bruit)
- As-tu un onduleur pour les coupures de courant ?
- Ta connexion internet est-elle assez stable pour des workflows automatises ?
- Peux-tu te connecter en SSH a ta machine a distance si quelque chose casse ?

Si tu ne peux pas tourner 24h/24, c'est pas grave — beaucoup de flux de revenus dans ce cours sont des jobs batch asynchrones que tu declenches manuellement. Mais ceux qui generent des revenus vraiment passifs necessitent de la disponibilite.

{? if computed.os_family == "windows" ?}
**Configuration rapide de la disponibilite (Windows) :** Utilise le Planificateur de taches pour le redemarrage automatique, active le Bureau a distance ou installe Tailscale pour l'acces distant, et configure ton BIOS sur "restaurer a la reprise du courant" pour recuperer apres les coupures.
{? endif ?}

**Configuration rapide de la disponibilite (si tu la veux) :**

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

### Les Maths de l'Electricite

Les gens ignorent ce sujet ou en font une catastrophe. Faisons de vrais calculs.

**Mesurer ta consommation reelle :**

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

**Calcul du cout mensuel :**

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
Ton tarif d'electricite : environ {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh (base sur les moyennes de {= regional.country | fallback("your region") =}). Verifie ta facture reelle — les tarifs varient selon le fournisseur et l'heure de la journee.
{? else ?}
La moyenne americaine est d'environ 0,12 $/kWh. Verifie ton tarif reel — il varie enormement. La Californie peut atteindre 0,25 $/kWh. Certains pays europeens atteignent 0,35 $/kWh. Certaines regions du Midwest americain sont a 0,08 $/kWh.
{? endif ?}

**Le point important :** Faire tourner ta machine 24h/24, 7j/7 pour generer des revenus coute entre {= regional.currency_symbol | fallback("$") =}1 et {= regional.currency_symbol | fallback("$") =}30/mois en electricite. Si tes flux de revenus ne peuvent pas couvrir ca, le probleme n'est pas l'electricite — c'est le flux de revenus.

### Specs Minimales par Type de Moteur de Revenus

Voici un apercu de la direction que nous prenons dans le cours STREETS complet. Pour l'instant, verifie simplement ou ta machine se situe :

| Moteur de Revenus | CPU | RAM | GPU | Stockage | Reseau |
|---------------|-----|-----|-----|---------|---------|
| **Automatisation de contenu** (articles de blog, newsletters) | 4+ coeurs | 16 Go | Optionnel (fallback API) | 50 Go libres | 10 Mbps montant |
| **Services de traitement de donnees** | 8+ coeurs | 32 Go | Optionnel | 200 Go libres | 50 Mbps montant |
| **Services API d'IA locale** | 8+ coeurs | 32 Go | 8+ Go VRAM | 100 Go libres | 50 Mbps montant |
| **Outils de generation de code** | 8+ coeurs | 16 Go | 8+ Go VRAM ou API | 50 Go libres | 10 Mbps montant |
| **Traitement de documents** | 4+ coeurs | 16 Go | Optionnel | 100 Go libres | 10 Mbps montant |
| **Agents autonomes** | 8+ coeurs | 32 Go | 12+ Go VRAM | 100 Go libres | 50 Mbps montant |

> **Erreur courante :** "Je dois mettre a niveau mon materiel avant de pouvoir commencer." Non. Commence avec ce que tu as. Utilise des appels API pour combler les lacunes que ton materiel ne peut pas couvrir. Mets a niveau quand les revenus le justifient — pas avant.

{@ insight engine_ranking @}

### Point de Controle de la Lecon 1

Tu devrais maintenant avoir note :
- [ ] Modele de CPU, coeurs et threads
- [ ] Quantite de RAM
- [ ] Modele de GPU et VRAM (ou "aucun")
- [ ] Stockage disponible
- [ ] Vitesses reseau (descendant/montant)
- [ ] Cout mensuel estime en electricite pour un fonctionnement 24h/24
- [ ] Pour quelles categories de moteurs de revenus ta machine est qualifiee

Garde ces chiffres. Tu les insereras dans ton Document de Stack Souveraine a la Lecon 6.

{? if computed.profile_completeness != "0" ?}
> **4DA a deja collecte la plupart de ces chiffres pour toi.** Consulte les resumes personnalises ci-dessus — ton inventaire materiel est partiellement pre-rempli grace a la detection systeme.
{? endif ?}

*Dans le cours STREETS complet, le Module R (Moteurs de Revenus) te donne des playbooks specifiques, etape par etape, pour chaque type de moteur liste ci-dessus — y compris le code exact pour les construire et les deployer.*

---

## Lecon 2 : Le Stack LLM Local

*"Configure Ollama pour un usage en production — pas juste pour le chat."*

### Pourquoi les LLM Locaux Comptent pour les Revenus

Chaque fois que tu appelles l'API OpenAI, tu paies un loyer. Chaque fois que tu fais tourner un modele localement, cette inference est gratuite apres la configuration initiale. Le calcul est simple :

- GPT-4o : ~5 $ par million de tokens d'entree, ~15 $ par million de tokens de sortie
- Claude 3.5 Sonnet : ~3 $ par million de tokens d'entree, ~15 $ par million de tokens de sortie
- Llama 3.1 8B local : 0 $ par million de tokens (juste l'electricite)

Si tu construis des services qui traitent des milliers de requetes, la difference entre 0 $ et 5-15 $ par million de tokens, c'est la difference entre le profit et l'equilibre.

Mais voici la nuance que la plupart des gens ratent : **les modeles locaux et les modeles API jouent des roles differents dans un stack de revenus.** Les modeles locaux gerent le volume. Les modeles API gerent la sortie de qualite critique, destinee aux clients. Ton stack a besoin des deux.

### Installer Ollama

{? if settings.has_llm ?}
> **Tu as deja un LLM configure :** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}. Si Ollama tourne deja, passe directement a "Guide de Selection de Modeles" ci-dessous.
{? endif ?}

Ollama est la fondation. Ca transforme ta machine en serveur d'inference local avec une API propre.

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
> **Windows :** Utilise l'installateur depuis ollama.com ou `winget install Ollama.Ollama`. Ollama tourne comme service en arriere-plan automatiquement apres l'installation.
{? elif computed.os_family == "macos" ?}
> **macOS :** `brew install ollama` est le chemin le plus rapide. Ollama exploite la memoire unifiee d'Apple Silicon — tes {= profile.ram.total | fallback("system") =} de RAM sont partages entre les charges CPU et GPU.
{? elif computed.os_family == "linux" ?}
> **Linux :** Le script d'installation gere tout. Si tu es sous {= profile.os.name | fallback("Linux") =}, Ollama s'installe comme service systemd.
{? endif ?}

Verifie l'installation :

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **Note sur la version :** Ollama sort des mises a jour frequemment. Les commandes de modeles et flags dans ce module ont ete verifies avec Ollama v0.5.x (debut 2026). Si tu lis ceci plus tard, consulte [ollama.com/download](https://ollama.com/download) pour la derniere version et [ollama.com/library](https://ollama.com/library) pour les noms de modeles actuels. Les concepts fondamentaux ne changent pas, mais les tags de modeles specifiques (par ex., `llama3.1:8b`) peuvent etre remplaces par des versions plus recentes.

### Guide de Selection de Modeles

Ne telecharge pas tous les modeles que tu vois. Sois strategique. Voici ce qu'il faut recuperer et quand utiliser chacun.

{? if computed.llm_tier ?}
> **Ta categorie LLM (basee sur le materiel) :** {= computed.llm_tier | fallback("unknown") =}. Les recommandations ci-dessous sont taguees pour que tu puisses te concentrer sur la categorie qui correspond a ta machine.
{? endif ?}

#### Categorie 1 : Le Cheval de Bataille (modeles 7B-8B)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**Utiliser pour :**
- Classification de texte ("Cet email est-il du spam ou legitime ?")
- Resume (condenser de longs documents en points cles)
- Extraction de donnees simple (extraire des noms, dates, montants d'un texte)
- Analyse de sentiment
- Etiquetage et categorisation de contenu
- Generation d'embeddings (si tu utilises un modele avec support d'embeddings)

**Performance (typique) :**
- RTX 3060 12GB : ~40-60 tokens/seconde
- RTX 4090 : ~100-130 tokens/seconde
- M2 Pro 16GB : ~30-45 tokens/seconde
- CPU uniquement (Ryzen 7 5800X) : ~8-12 tokens/seconde

**Comparaison de couts :**
- 1 million de tokens via GPT-4o-mini : ~0,60 $
- 1 million de tokens localement (modele 8B) : ~0,003 $ en electricite
- Seuil de rentabilite : ~5 000 tokens (tu economises de l'argent des la toute premiere requete)

#### Categorie 2 : Le Choix Equilibre (modeles 13B-14B)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**Utiliser pour :**
- Redaction de contenu (articles de blog, documentation, textes marketing)
- Generation de code (fonctions, scripts, boilerplate)
- Transformation de donnees complexe
- Taches de raisonnement en plusieurs etapes
- Traduction avec nuance

**Performance (typique) :**
- RTX 3060 12GB : ~20-30 tokens/seconde (quantifie)
- RTX 4090 : ~60-80 tokens/seconde
- M2 Pro 32GB : ~20-30 tokens/seconde
- CPU uniquement : ~3-6 tokens/seconde (pas pratique pour le temps reel)

**Quand utiliser plutot que le 7B :** Quand la qualite de sortie du 7B n'est pas suffisante mais que tu n'as pas besoin de payer pour des appels API. Teste les deux sur ton cas d'usage reel — parfois le 7B suffit et tu gaspilles juste de la puissance de calcul.

{? if computed.gpu_tier == "capable" ?}
> **Territoire extensible vers la Categorie 3** — Ton {= profile.gpu.model | fallback("GPU") =} peut gerer du 30B quantifie avec un peu d'effort, mais le 70B est hors de portee localement. Envisage des appels API pour les taches qui necessitent une qualite de niveau 70B.
{? endif ?}

#### Categorie 3 : La Categorie Qualite (modeles 30B-70B)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**Utiliser pour :**
- Contenu destine aux clients qui doit etre excellent
- Analyse et raisonnement complexes
- Generation de contenu long format
- Taches ou la qualite impacte directement si quelqu'un te paie ou non

**Performance (typique) :**
- RTX 4090 (24GB) : 70B a ~8-15 tokens/seconde (utilisable mais lent)
- Double GPU ou 48Go+ : 70B a ~20-30 tokens/seconde
- M3 Max 64GB : 70B a ~10-15 tokens/seconde

> **Parlons franchement :** Si tu n'as pas 24Go+ de VRAM, oublie completement les modeles 70B. Utilise des appels API pour la sortie critique en qualite. Un modele 70B qui tourne a 3 tokens/seconde depuis la RAM systeme est techniquement possible mais pratiquement inutile pour tout workflow generateur de revenus. Ton temps a de la valeur.

#### Categorie 4 : Les Modeles API (Quand le Local Ne Suffit Pas)

Les modeles locaux sont pour le volume et la confidentialite. Les modeles API sont pour les plafonds de qualite et les capacites specialisees.

**Quand utiliser les modeles API :**
- Sortie destinee aux clients ou qualite = revenus (textes de vente, contenu premium)
- Chaines de raisonnement complexes que les modeles plus petits echouent
- Taches de vision/multimodal (analyse d'images, captures d'ecran, documents)
- Quand tu as besoin d'une sortie JSON structuree avec une haute fiabilite
- Quand la vitesse compte et que ton materiel local est lent

**Tableau de comparaison des couts (debut 2025 — verifie les prix actuels) :**

| Modele | Entree (par 1M tokens) | Sortie (par 1M tokens) | Ideal Pour |
|-------|----------------------|------------------------|----------|
| GPT-4o-mini | 0,15 $ | 0,60 $ | Travail de volume a bas cout (quand le local n'est pas disponible) |
| GPT-4o | 2,50 $ | 10,00 $ | Vision, raisonnement complexe |
| Claude 3.5 Sonnet | 3,00 $ | 15,00 $ | Code, analyse, contexte long |
| Claude 3.5 Haiku | 0,80 $ | 4,00 $ | Rapide, pas cher, bon equilibre qualite |
| DeepSeek V3 | 0,27 $ | 1,10 $ | Budget-friendly, performance solide |

**La strategie hybride :**
1. Le LLM local 7B/13B gere 80% des requetes (classification, extraction, resume)
2. L'API gere 20% des requetes (passe qualite finale, taches complexes, sortie destinee aux clients)
3. Ton cout effectif : ~0,50-2,00 $ par million de tokens en melange (au lieu de 5-15 $ en pur API)

Cette approche hybride est la facon de construire des services avec des marges saines. Plus de details dans le Module R.

### Configuration Production

Faire tourner Ollama pour du travail generateur de revenus est different de le faire tourner pour du chat personnel. Voici comment le configurer correctement.

{? if computed.has_nvidia ?}
> **GPU NVIDIA detecte ({= profile.gpu.model | fallback("unknown") =}).** Ollama utilisera automatiquement l'acceleration CUDA. Assure-toi que tes pilotes NVIDIA sont a jour — lance `nvidia-smi` pour verifier. Pour une performance optimale avec {= profile.gpu.vram | fallback("your") =} de VRAM, le parametre `OLLAMA_MAX_LOADED_MODELS` ci-dessous devrait correspondre au nombre de modeles qui tiennent simultanement dans ta VRAM.
{? endif ?}

#### Definir les Variables d'Environnement

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

#### Creer un Modelfile pour ta Charge de Travail

Au lieu d'utiliser les parametres par defaut du modele, cree un Modelfile personnalise ajuste pour ta charge de travail generatrice de revenus :

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

#### Gestion du Batching et des Files d'Attente

Pour les charges de travail generatrices de revenus, tu auras souvent besoin de traiter beaucoup d'elements. Voici une configuration de batching de base :

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

### Benchmarker TA Machine

Ne fais confiance aux benchmarks de personne d'autre. Mesure les tiens :

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

Note tes tokens/seconde pour chaque modele. Ce chiffre determine quels workflows de revenus sont pratiques pour ta machine.

{@ insight stack_fit @}

**Exigences de vitesse par cas d'usage :**
- Traitement batch (asynchrone) : 5+ tokens/sec suffit (tu ne te soucies pas de la latence)
- Outils interactifs (l'utilisateur attend) : 20+ tokens/sec minimum
- API temps reel (destine aux clients) : 30+ tokens/sec pour une bonne UX
- Chat en streaming : 15+ tokens/sec parait reactif

### Securiser Ton Serveur d'Inference Local

{? if computed.os_family == "windows" ?}
> **Note Windows :** Ollama sur Windows se lie a localhost par defaut. Verifie avec `netstat -an | findstr 11434` dans PowerShell. Utilise le Pare-feu Windows pour bloquer l'acces externe au port 11434.
{? elif computed.os_family == "macos" ?}
> **Note macOS :** Ollama sur macOS se lie a localhost par defaut. Verifie avec `lsof -i :11434`. Le pare-feu macOS devrait bloquer les connexions externes automatiquement.
{? endif ?}

Ton instance Ollama ne devrait jamais etre accessible depuis internet a moins que tu ne le veuilles explicitement.

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

> **Erreur courante :** Lier Ollama a 0.0.0.0 par "commodite" et l'oublier. N'importe qui trouvant ton IP peut utiliser ton GPU pour de l'inference gratuite. Pire, ils peuvent extraire les poids du modele et les prompts systeme. Toujours localhost. Toujours tunnel.

### Point de Controle de la Lecon 2

Tu devrais maintenant avoir :
- [ ] Ollama installe et en cours d'execution
- [ ] Au moins un modele cheval de bataille telecharge (llama3.1:8b ou equivalent)
- [ ] Un Modelfile personnalise pour ta charge de travail prevue
- [ ] Des chiffres de benchmark : tokens/seconde pour chaque modele sur ta machine
- [ ] Ollama lie a localhost uniquement

*Dans le cours STREETS complet, le Module T (Avantages Techniques) te montre comment construire des configurations de modeles proprietaires, des pipelines de fine-tuning et des toolchains personnalisees que les concurrents ne peuvent pas facilement repliquer. Le Module R (Moteurs de Revenus) te donne les services exacts a construire par-dessus ce stack.*

---

## Lecon 3 : L'Avantage Confidentialite

*"Ta configuration privee EST un avantage concurrentiel — pas juste une preference."*

### La Confidentialite Est une Fonctionnalite Produit, Pas une Limitation

La plupart des developpeurs mettent en place une infrastructure locale parce qu'ils valorisent personnellement la confidentialite, ou parce qu'ils aiment bidouiller. C'est bien. Mais tu laisses de l'argent sur la table si tu ne realises pas que **la confidentialite est l'une des fonctionnalites les plus commercialisables en tech en ce moment.**

Voici pourquoi : chaque fois qu'une entreprise envoie des donnees a l'API d'OpenAI, ces donnees passent par un tiers. Pour beaucoup d'entreprises — surtout celles dans la sante, la finance, le juridique, le gouvernement et les entreprises basees dans l'UE — c'est un vrai probleme. Pas un probleme theorique. Un probleme du type "on ne peut pas utiliser cet outil parce que la conformite a dit non".

Toi, qui fais tourner des modeles localement sur ta machine, tu n'as pas ce probleme.

### Le Vent Reglementaire Favorable

L'environnement reglementaire evolue dans ta direction. Rapidement.

{? if regional.country == "US" ?}
> **Base aux Etats-Unis :** Les reglementations ci-dessous qui te concernent le plus sont HIPAA, SOC 2, ITAR et les lois de confidentialite au niveau des Etats (California CCPA, etc.). Les reglementations de l'UE comptent quand meme — elles affectent ta capacite a servir des clients europeens, ce qui est un marche lucratif.
{? elif regional.country == "GB" ?}
> **Base au Royaume-Uni :** Post-Brexit, le Royaume-Uni a son propre cadre de protection des donnees (UK GDPR + Data Protection Act 2018). Ton avantage de traitement local est particulierement fort pour servir les services financiers britanniques et le travail adjacent au NHS.
{? elif regional.country == "DE" ?}
> **Base en Allemagne :** Tu es dans l'un des environnements de protection des donnees les plus stricts au monde. C'est un *avantage* — les clients allemands comprennent deja pourquoi le traitement local est important, et ils paieront pour ca.
{? elif regional.country == "AU" ?}
> **Base en Australie :** Le Privacy Act 1988 et les Australian Privacy Principles (APPs) regissent tes obligations. Le traitement local est un argument de vente fort pour les clients gouvernementaux et de sante sous le My Health Records Act.
{? endif ?}

**EU AI Act (applique de 2024 a 2026) :**
- Les systemes d'IA a haut risque necessitent des pipelines de traitement de donnees documentes
- Les entreprises doivent demontrer ou vont les donnees et qui les traite
- Le traitement local simplifie la conformite de facon dramatique
- Les entreprises de l'UE cherchent activement des fournisseurs de services IA qui peuvent garantir la residence des donnees dans l'UE

**RGPD (deja applique) :**
- Le "traitement de donnees" inclut l'envoi de texte a une API LLM
- Les entreprises ont besoin d'Accords de Traitement de Donnees avec chaque tiers
- Le traitement local elimine entierement le tiers
- C'est un vrai argument de vente : "Vos donnees ne quittent jamais votre infrastructure. Il n'y a pas de DPA tiers a negocier."

**Reglementations sectorielles :**
- **HIPAA (Sante US) :** Les donnees patient ne peuvent pas etre envoyees a des API d'IA grand public sans un BAA (Business Associate Agreement). La plupart des fournisseurs d'IA ne proposent pas de BAA pour l'acces API. Le traitement local contourne entierement ce probleme.
- **SOC 2 (Entreprise) :** Les entreprises en audit SOC 2 doivent documenter chaque processeur de donnees. Moins de processeurs = audits plus faciles.
- **ITAR (Defense US) :** Les donnees techniques controlees ne peuvent pas quitter la juridiction americaine. Les fournisseurs d'IA cloud avec une infrastructure internationale posent probleme.
- **PCI DSS (Finance) :** Le traitement des donnees de carte bancaire a des exigences strictes sur la circulation des donnees.

### Comment Positionner la Confidentialite dans les Conversations de Vente

Tu n'as pas besoin d'etre un expert en conformite. Tu dois comprendre trois phrases et savoir quand les utiliser :

**Phrase 1 : "Vos donnees ne quittent jamais votre infrastructure."**
A utiliser quand : Tu parles a tout prospect soucieux de la confidentialite. C'est l'accroche universelle.

**Phrase 2 : "Aucun accord de traitement de donnees tiers requis."**
A utiliser quand : Tu parles a des entreprises europeennes ou a toute entreprise avec une equipe juridique/conformite. Ca leur economise des semaines de revue juridique.

**Phrase 3 : "Audit trail complet, traitement mono-tenant."**
A utiliser quand : Tu parles a des entreprises ou des industries reglementees. Ils doivent prouver leur pipeline IA aux auditeurs.

**Exemple de positionnement (pour ta page de service ou tes propositions) :**

> "Contrairement aux services d'IA bases dans le cloud, [Ton Service] traite toutes les donnees localement sur du materiel dedie. Tes documents, ton code et tes donnees ne quittent jamais l'environnement de traitement. Il n'y a pas d'API tiers dans le pipeline, pas d'accords de partage de donnees a negocier, et une journalisation d'audit complete de chaque operation. Cela rend [Ton Service] adapte aux organisations ayant des exigences strictes de traitement de donnees, y compris les environnements de conformite RGPD, HIPAA et SOC 2."

Ce paragraphe, sur une landing page, attirera exactement les clients qui paieront des tarifs premium.

### La Justification du Tarif Premium

Voici le business case en chiffres concrets :

**Service de traitement IA standard (utilisant des API cloud) :**
- Les donnees du client vont a OpenAI/Anthropic/Google
- Tu es en concurrence avec chaque developpeur qui sait appeler une API
- Tarif du marche : 0,01-0,05 $ par document traite
- Tu revends essentiellement de l'acces API avec une marge

**Service de traitement IA confidentialite-first (ton stack local) :**
- Les donnees du client restent sur ta machine
- Tu es en concurrence avec un pool de fournisseurs beaucoup plus restreint
- Tarif du marche : 0,10-0,50 $ par document traite (premium de 5-10x)
- Tu vends de l'infrastructure + de l'expertise + de la conformite

Le premium confidentialite est reel : **5x a 10x** par rapport aux services cloud commoditises pour la meme tache sous-jacente. Et les clients qui le paient sont plus fideles, moins sensibles aux prix, et ont des budgets plus importants.

{@ insight competitive_position @}

### Mettre en Place des Espaces de Travail Isoles

Si tu as un emploi principal (la plupart d'entre vous en ont un), tu as besoin d'une separation nette entre le travail employeur et le travail generateur de revenus. Ce n'est pas juste une protection juridique — c'est de l'hygiene operationnelle.

{? if computed.os_family == "windows" ?}
> **Astuce Windows :** Cree un compte utilisateur Windows separe pour le travail generateur de revenus (Parametres > Comptes > Famille et autres utilisateurs > Ajouter quelqu'un d'autre). Ca te donne un environnement completement isole — profils de navigateur separes, chemins de fichiers separes, variables d'environnement separees. Bascule entre les comptes avec Win+L.
{? endif ?}

**Option 1 : Comptes utilisateur separes (recommande)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**Option 2 : Espaces de travail containerises**

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

**Option 3 : Machine physique separee (le plus inattaquable)**

Si tu es serieux a ce sujet et que tes revenus le justifient, une machine dediee elimine toutes les questions. Un Dell OptiPlex d'occasion avec une RTX 3060 coute 400-600 $ et se rembourse le premier mois de travail client.

**Checklist de separation minimale :**
- [ ] Projets generateurs de revenus dans un repertoire separe (jamais melanges avec les repos employeur)
- [ ] Cles API separees pour le travail de revenus (n'utilise jamais les cles fournies par l'employeur)
- [ ] Profil de navigateur separe pour les comptes lies aux revenus
- [ ] Le travail de revenus n'est jamais fait sur le materiel de l'employeur
- [ ] Le travail de revenus n'est jamais fait sur le reseau de l'employeur (utilise ton internet personnel ou un VPN)
- [ ] Compte GitHub/GitLab separe pour les projets de revenus (optionnel mais propre)

> **Erreur courante :** Utiliser la cle API OpenAI de ton employeur "juste pour tester" ton projet perso. Ca cree une trace papier que le tableau de bord de facturation de ton employeur peut voir, et ca brouille les eaux en matiere de propriete intellectuelle. Obtiens tes propres cles. Elles sont pas cheres.

### Point de Controle de la Lecon 3

Tu devrais maintenant comprendre :
- [ ] Pourquoi la confidentialite est une fonctionnalite produit commercialisable, pas juste une preference personnelle
- [ ] Quelles reglementations creent de la demande pour le traitement IA local
- [ ] Trois phrases a utiliser dans les conversations de vente sur la confidentialite
- [ ] Comment les services confidentialite-first commandent un tarif premium de 5-10x
- [ ] Comment separer le travail de revenus du travail employeur

*Dans le cours STREETS complet, le Module E (Avantage Evolutif) t'apprend a suivre les changements reglementaires et a te positionner en avance sur les nouvelles exigences de conformite avant que tes concurrents ne sachent meme qu'elles existent.*

---

## Lecon 4 : Le Minimum Legal

*"Quinze minutes de configuration juridique maintenant evitent des mois de problemes plus tard."*

### Ceci N'Est Pas un Conseil Juridique

Je suis developpeur, pas avocat. Ce qui suit est une checklist pratique que la plupart des developpeurs dans la plupart des situations devraient traiter. Si ta situation est complexe (participation au capital de ton employeur, clause de non-concurrence avec des termes specifiques, etc.), depense 200 $ pour une consultation de 30 minutes avec un avocat en droit du travail. C'est le meilleur retour sur investissement que tu obtiendras.

### Etape 1 : Lis Ton Contrat de Travail

Trouve ton contrat de travail ou ta lettre d'offre. Cherche ces sections :

**Clause d'attribution de propriete intellectuelle** — Cherche des formulations comme :
- "Toutes les inventions, developpements et travaux..."
- "...crees pendant la duree de l'emploi..."
- "...lies a l'activite de l'Entreprise ou a son activite anticipee..."

**Phrases cles qui te restreignent :**
- "Tout travail cree pendant l'emploi appartient a l'Entreprise" (large — potentiellement problematique)
- "Travaux crees avec les ressources de l'Entreprise" (plus restreint — generalement ok si tu utilises ton propre equipement)
- "Lie a l'activite actuelle ou anticipee de l'Entreprise" (depend de ce que fait ton employeur)

**Phrases cles qui te liberent :**
- "A l'exclusion du travail effectue entierement sur le temps propre de l'Employe avec ses propres ressources et sans rapport avec l'activite de l'Entreprise" (c'est ton exemption — de nombreux Etats americains l'exigent)
- Certains Etats (Californie, Washington, Minnesota, Illinois, et d'autres) ont des lois qui limitent les revendications IP de l'employeur sur les projets personnels, quel que soit ce que dit le contrat.

### Le Test des 3 Questions

Pour tout projet generateur de revenus, demande-toi :

1. **Temps :** Fais-tu ce travail sur ton propre temps ? (Pas pendant les heures de travail, pas pendant les astreintes)
2. **Equipement :** Utilises-tu ton propre materiel, ton propre internet, tes propres cles API ? (Pas le portable de l'employeur, pas le VPN de l'employeur, pas les comptes cloud de l'employeur)
3. **Sujet :** Est-ce sans rapport avec l'activite de ton employeur ? (Si tu travailles dans une boite d'IA sante et que tu veux vendre des services d'IA sante... c'est un probleme. Si tu travailles dans une boite d'IA sante et que tu veux vendre du traitement de documents pour des agents immobiliers... c'est ok.)

Si les trois reponses sont nettes, tu es presque certainement tranquille. Si une reponse est floue, clarifie avant de continuer.

> **Parlons franchement :** La grande majorite des developpeurs qui font du travail annexe n'ont jamais de probleme. Les employeurs se soucient de proteger leurs avantages concurrentiels, pas de t'empecher de gagner de l'argent supplementaire sur des projets sans rapport. Mais "presque certainement tranquille" n'est pas "definitivement tranquille." Si ton contrat est inhabituellement large, aie une conversation avec ton manager ou les RH — ou consulte un avocat. Le risque de ne pas verifier est bien pire que la legere gene de poser la question.

### Etape 2 : Choisis une Structure Juridique

Tu as besoin d'une entite juridique pour separer tes actifs personnels de tes activites commerciales, et pour ouvrir la porte aux services bancaires professionnels, au traitement des paiements et aux avantages fiscaux.

{? if regional.country ?}
> **Ta localisation : {= regional.country | fallback("Unknown") =}.** Le type d'entite recommande pour ta region est une **{= regional.business_entity_type | fallback("LLC or equivalent") =}**, avec des couts d'inscription typiques de {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Fais defiler jusqu'a la section de ton pays ci-dessous, ou lis toutes les sections pour comprendre comment les clients d'autres regions fonctionnent.
{? endif ?}

{? if regional.country == "US" ?}
#### Etats-Unis (Ta Region)
{? else ?}
#### Etats-Unis
{? endif ?}

| Structure | Cout | Protection | Ideal Pour |
|-----------|------|------------|----------|
| **Sole Proprietorship** (par defaut) | 0 $ | Aucune (responsabilite personnelle) | Tater le terrain. Les premiers 1 000 $. |
| **Single-Member LLC** | 50-500 $ (varie selon l'Etat) | Protection des actifs personnels | Travail de revenus actif. La plupart des developpeurs devraient commencer ici. |
| **Election S-Corp** (sur une LLC) | Cout LLC + 0 $ pour l'election | Meme que LLC + avantages charges sociales | Quand tu gagnes regulierement 40 000 $+/an avec ca |

**Recommande pour les developpeurs US :** Single-Member LLC dans ton Etat de residence.

**Etats les moins chers pour la creation :** Wyoming (100 $, pas d'impot d'Etat sur le revenu), New Mexico (50 $), Montana (70 $). Mais creer dans ton Etat de residence est generalement plus simple sauf si tu as une raison specifique de ne pas le faire.

**Comment deposer :**
1. Va sur le site web du Secretary of State de ton Etat
2. Cherche "form LLC" ou "business entity filing"
3. Depose les Articles of Organization (formulaire de 10 minutes)
4. Obtiens un EIN aupres de l'IRS (gratuit, prend 5 minutes sur irs.gov)

{? if regional.country == "GB" ?}
#### Royaume-Uni (Ta Region)
{? else ?}
#### Royaume-Uni
{? endif ?}

| Structure | Cout | Protection | Ideal Pour |
|-----------|------|------------|----------|
| **Sole Trader** | Gratuit (inscription HMRC) | Aucune | Premiers revenus. Test. |
| **Limited Company (Ltd)** | ~15 $ via Companies House | Protection des actifs personnels | Tout travail de revenus serieux. |

**Recommande :** Ltd company via Companies House. Ca prend environ 20 minutes et coute 12 GBP.

#### Union Europeenne

Varie considerablement selon le pays, mais le schema general :

- **Allemagne :** Einzelunternehmer (auto-entrepreneur) pour commencer, GmbH pour le serieux (mais la GmbH necessite 25 000 EUR de capital — envisage une UG pour 1 EUR)
- **Pays-Bas :** Eenmanszaak (auto-entrepreneur, inscription gratuite) ou BV (comparable a une Ltd)
- **France :** Micro-entrepreneur (simplifie, recommande pour demarrer)
- **Estonie :** e-Residency + OUE (populaire pour les non-residents, entierement en ligne)

{? if regional.country == "AU" ?}
#### Australie (Ta Region)
{? else ?}
#### Australie
{? endif ?}

| Structure | Cout | Protection | Ideal Pour |
|-----------|------|------------|----------|
| **Sole Trader** | ABN gratuit | Aucune | Pour demarrer |
| **Pty Ltd** | ~500-800 AUD via ASIC | Protection des actifs personnels | Revenus serieux |

**Recommande :** Commence avec un Sole Trader ABN (gratuit, instantane), passe a Pty Ltd quand tu gagnes regulierement.

### Etape 3 : Traitement des Paiements (configuration en 15 minutes)

Tu as besoin d'un moyen de te faire payer. Configure ca maintenant, pas quand ton premier client attend.

{? if regional.payment_processors ?}
> **Recommande pour {= regional.country | fallback("your region") =} :** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (recommande pour la plupart des developpeurs) :**

```
1. Go to stripe.com
2. Create account with your business email
3. Complete identity verification
4. Connect your business bank account
5. You can now accept payments, create invoices, and set up subscriptions
```

Duree : ~15 minutes. Tu peux commencer a accepter des paiements immediatement (Stripe retient les fonds pendant 7 jours pour les nouveaux comptes).

**Lemon Squeezy (recommande pour les produits numeriques) :**

Si tu vends des produits numeriques (templates, outils, cours, SaaS), Lemon Squeezy agit comme ton Merchant of Record. Ca signifie :
- Ils gerent la taxe de vente, la TVA et la GST pour toi a l'echelle mondiale
- Tu n'as pas besoin de t'inscrire a la TVA dans l'UE
- Ils gerent les remboursements et les litiges

```
1. Go to lemonsqueezy.com
2. Create account
3. Set up your store
4. Add products
5. They handle everything else
```

**Stripe Atlas (pour les developpeurs internationaux ou ceux voulant une entite US) :**

Si tu es hors des US mais veux vendre a des clients americains avec une entite US :
- 500 $ de frais uniques
- Cree une Delaware LLC pour toi
- Ouvre un compte bancaire US (via Mercury ou Stripe)
- Fournit un service d'agent enregistre
- Prend environ 1-2 semaines

### Etape 4 : Politique de Confidentialite et Conditions d'Utilisation

Si tu vends un service ou un produit en ligne, tu en as besoin. Ne paie pas un avocat pour du boilerplate.

**Sources gratuites et reputees pour les modeles :**
- **Termly.io** — Generateur gratuit de politique de confidentialite et de CGU. Reponds aux questions, obtiens les documents.
- **Avodocs.com** — Documents juridiques open-source pour startups. Gratuit.
- **choosealicense.com de GitHub** — Pour les licences de projets open-source specifiquement.
- **Politiques open-sourcees de Basecamp** — Cherche "Basecamp open source policies" — de bons modeles en langage clair.

**Ce que ta politique de confidentialite doit couvrir (si tu traites des donnees client) :**
- Quelles donnees tu collectes
- Comment tu les traites (localement — c'est ton avantage)
- Combien de temps tu les conserves
- Comment les clients peuvent demander la suppression
- Si des tiers accedent aux donnees (idealement : aucun)

**Duree :** 30 minutes avec un generateur de modeles. Fait.

### Etape 5 : Compte Bancaire Separe

Ne fais pas transiter les revenus professionnels par ton compte courant personnel. Les raisons :

1. **Clarte fiscale :** Quand vient le moment des impots, tu dois savoir exactement ce qui etait un revenu professionnel et ce qui ne l'etait pas.
2. **Protection juridique :** Si tu as une LLC, melanger les fonds personnels et professionnels peut "percer le voile corporatif" — ce qui signifie qu'un tribunal peut ignorer la protection de responsabilite de ta LLC.
3. **Professionnalisme :** Des factures de "Consulting de Jean SARL" arrivant sur un compte professionnel dedie, ca fait serieux. Des paiements sur ton Venmo personnel, non.

**Services bancaires professionnels gratuits ou a bas cout :**
{? if regional.country == "US" ?}
- **Mercury** (recommande pour toi) — Gratuit, concu pour les startups. Excellente API si tu veux automatiser la comptabilite plus tard.
- **Relay** — Gratuit, bien pour separer les flux de revenus en sous-comptes.
{? elif regional.country == "GB" ?}
- **Starling Bank** (recommande pour toi) — Compte professionnel gratuit, creation instantanee.
- **Wise Business** — Multi-devises a bas cout. Ideal si tu sers des clients internationaux.
{? else ?}
- **Mercury** (US) — Gratuit, concu pour les startups. Excellente API si tu veux automatiser la comptabilite plus tard.
- **Relay** (US) — Gratuit, bien pour separer les flux de revenus en sous-comptes.
- **Starling Bank** (UK) — Compte professionnel gratuit.
{? endif ?}
- **Wise Business** (International) — Multi-devises a bas cout. Ideal pour recevoir des paiements en USD, EUR, GBP, etc.
- **Qonto** (UE) — Services bancaires professionnels elegants pour les entreprises europeennes.

Ouvre le compte maintenant. Ca prend 10-15 minutes en ligne et 1-3 jours pour la verification.

### Etape 6 : Bases Fiscales pour les Revenus Secondaires de Developpeur

{? if regional.tax_note ?}
> **Note fiscale pour {= regional.country | fallback("your region") =} :** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **Parlons franchement :** Les impots sont le truc que la plupart des developpeurs ignorent jusqu'en avril, puis paniquent. Passer 30 minutes maintenant t'economise de l'argent reel et du stress.

**Etats-Unis :**
- Les revenus secondaires au-dessus de 400 $/an necessitent une taxe sur le travail independant (~15,3% pour Social Security + Medicare)
- Plus ta tranche d'imposition ordinaire sur le benefice net
- **Acomptes trimestriels :** Si tu dois plus de 1 000 $ d'impots, l'IRS attend des paiements trimestriels (15 avril, 15 juin, 15 sept, 15 janv). Le sous-paiement entraine des penalites.
- Mets de cote **25-30%** du revenu net pour les impots. Place-le immediatement sur un compte d'epargne separe.

**Deductions courantes pour les revenus secondaires de developpeur :**
- Couts API (OpenAI, Anthropic, etc.) — 100% deductibles
- Achats de materiel utilise pour l'activite — amortissables ou deduction Section 179
- Cout d'electricite attribuable a l'usage professionnel
- Abonnements logiciels utilises pour le travail generateur de revenus
- Deduction bureau a domicile (simplifie : 5 $/pi2, jusqu'a 300 pi2 = 1 500 $)
- Internet (pourcentage d'usage professionnel)
- Noms de domaine, hebergement, services email
- Developpement professionnel (cours, livres) lies a ton travail generateur de revenus

**Royaume-Uni :**
- Declaration via Self Assessment tax return
- Revenus commerciaux sous 1 000 GBP : exoneres d'impot (Trading Allowance)
- Au-dessus : paie l'Income Tax + Class 4 NICs sur les profits
- Dates de paiement : 31 janvier et 31 juillet

**Suis tout des le premier jour.** Utilise un simple tableur si rien d'autre :

```
| Date       | Category    | Description          | Amount  | Type    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Anthropic credit     | -$20.00 | Expense |
| 2025-01-18 | Revenue     | Client invoice #001  | +$500.00| Income  |
| 2025-01-20 | Software    | Vercel Pro plan      | -$20.00 | Expense |
| 2025-01-20 | Tax Reserve | 30% of net income    | -$138.00| Transfer|
```

> **Erreur courante :** "Je m'occuperai des impots plus tard." Plus tard c'est le Q4, tu dois 3 000 $ d'acomptes plus des penalites, et tu as deja depense l'argent. Automatise : chaque fois qu'un revenu arrive sur ton compte professionnel, transfere 30% vers un compte d'epargne fiscal immediatement.

### Point de Controle de la Lecon 4

Tu devrais maintenant avoir (ou avoir un plan pour) :
- [ ] Lu la clause de PI de ton contrat de travail
- [ ] Passe le Test des 3 Questions pour ton travail de revenus prevu
- [ ] Choisi une structure juridique (ou decide de commencer en auto-entrepreneur)
- [ ] Traitement des paiements configure (Stripe ou Lemon Squeezy)
- [ ] Politique de confidentialite et CGU a partir d'un generateur de modeles
- [ ] Compte bancaire professionnel separe (ou demande soumise)
- [ ] Strategie fiscale : mise de cote de 30% + calendrier de paiements trimestriels

*Dans le cours STREETS complet, le Module E (Execution Playbook) inclut des modeles de modelisation financiere qui calculent automatiquement tes obligations fiscales, la rentabilite des projets et les seuils de rentabilite pour chaque moteur de revenus.*

---

## Lecon 5 : Le Budget de {= regional.currency_symbol | fallback("$") =}200/mois

*"Ton activite a un burn rate. Connais-le. Controle-le. Fais-le rapporter."*

### Pourquoi {= regional.currency_symbol | fallback("$") =}200/mois

Deux cents {= regional.currency | fallback("dollars") =} par mois est le budget minimum viable pour une activite de revenus developpeur. C'est assez pour faire tourner de vrais services, servir de vrais clients et generer de vrais revenus. C'est aussi suffisamment petit pour que si rien ne marche, tu n'as pas mise la ferme.

L'objectif est simple : **transformer {= regional.currency_symbol | fallback("$") =}200/mois en {= regional.currency_symbol | fallback("$") =}600+/mois dans les 90 jours.** Si tu y arrives, tu as une activite. Si tu n'y arrives pas, tu changes de strategie — pas d'augmentation de budget.

### La Repartition du Budget

#### Categorie 1 : Credits API — 50-100 $/mois

C'est ta puissance de calcul de production pour la qualite destinee aux clients.

**Allocation de demarrage recommandee :**

```
Anthropic (Claude):     $40/month  — Your primary for quality output
OpenAI (GPT-4o-mini):   $20/month  — Cheap volume work, fallback
DeepSeek:               $10/month  — Budget tasks, experimentation
Buffer:                 $30/month  — Overflow or new provider testing
```

**Comment gerer les depenses API :**

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

**La strategie de depense hybride :**
- Utilise les LLM locaux pour 80% du traitement (classification, extraction, resume, brouillons)
- Utilise les appels API pour 20% du traitement (passe qualite finale, raisonnement complexe, sortie destinee aux clients)
- Ton cout effectif par tache baisse dramatiquement par rapport a l'usage pur API

{? if computed.monthly_electricity_estimate ?}
> **Ton cout d'electricite estime :** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/mois pour un fonctionnement 24h/24 a {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh. C'est deja pris en compte dans ton cout operationnel effectif.
{? endif ?}

#### Categorie 2 : Infrastructure — {= regional.currency_symbol | fallback("$") =}30-50/mois

```
Domain name:            $12/year ($1/month)     — Namecheap, Cloudflare, Porkbun
Email (business):       $0-6/month              — Zoho Mail free, or Google Workspace $6
VPS (optional):         $5-20/month             — For hosting lightweight services
                                                  Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/month                — Cloudflare free tier
Hosting (static):       $0/month                — Vercel, Netlify, Cloudflare Pages (free tiers)
```

**As-tu besoin d'un VPS ?**

Si ton modele de revenus est :
- **Vente de produits numeriques :** Non. Heberge sur Vercel/Netlify gratuitement. Utilise Lemon Squeezy pour la livraison.
- **Traitement asynchrone pour des clients :** Peut-etre. Tu peux faire tourner des jobs sur ta machine locale et livrer les resultats. Un VPS ajoute de la fiabilite.
- **Offrir un service API :** Oui, probablement. Un VPS a 5-10 $ sert de passerelle API legere, meme si le traitement lourd se fait sur ta machine locale.
- **Vente de SaaS :** Oui. Mais commence avec le tier le moins cher et monte en puissance.

**Infrastructure de demarrage recommandee :**

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

Cout total d'infrastructure : 5-20 $/mois. Le reste est en tiers gratuits.

#### Categorie 3 : Outils — {= regional.currency_symbol | fallback("$") =}20-30/mois

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

> **Parlons franchement :** Tu peux faire tourner ton stack d'outils entier sur des tiers gratuits au demarrage. Les 20-30 $ alloues ici sont pour quand tu depasses les tiers gratuits ou que tu veux une fonctionnalite premium specifique. Ne les depense pas juste parce qu'ils sont dans le budget. Le budget non depense, c'est du profit.

#### Categorie 4 : Reserve — {= regional.currency_symbol | fallback("$") =}0-30/mois

C'est ton fonds "choses que je n'ai pas prevues" :
- Un pic de cout API suite a un job batch plus gros que prevu
- Un outil dont tu as besoin pour un projet client specifique
- Achat d'urgence d'un nom de domaine quand tu trouves le nom parfait
- Un achat ponctuel (theme, template, set d'icones)

Si tu n'utilises pas la reserve, elle s'accumule. Apres 3 mois de reserve inutilisee, envisage de la reallouer aux credits API ou a l'infrastructure.

### Le Calcul du ROI

C'est le seul chiffre qui compte :

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

**Quand augmenter le budget :**

Augmente ton budget UNIQUEMENT quand :
1. Tu es regulierement a 2x+ de ROI depuis 2+ mois
2. Depenser plus augmenterait directement les revenus (par ex., plus de credits API = plus de capacite client)
3. L'augmentation est liee a un flux de revenus specifique et teste

**Quand NE PAS augmenter le budget :**
- "Je pense que ce nouvel outil va m'aider" (teste d'abord les alternatives gratuites)
- "Tout le monde dit qu'il faut depenser de l'argent pour en gagner" (pas a ce stade)
- "Un plus gros VPS rendra mon service plus rapide" (la vitesse est-elle vraiment le goulot d'etranglement ?)
- Tu n'as pas encore atteint 1x de ROI (corrige les revenus, pas les depenses)

**L'echelle de montee en puissance :**

```
$200/month  → Proving the concept (months 1-3)
$500/month  → Scaling what works (months 4-6)
$1000/month → Multiple revenue streams (months 6-12)
$2000+/month → Full business operation (year 2+)

Each step requires proving ROI at the current level first.
```

> **Erreur courante :** Traiter les {= regional.currency_symbol | fallback("$") =}200 comme un "investissement" qui n'a pas besoin de rapporter de l'argent immediatement. Non. C'est une experience avec une echeance de 90 jours. Si {= regional.currency_symbol | fallback("$") =}200/mois ne genere pas {= regional.currency_symbol | fallback("$") =}200/mois de revenus en 90 jours, quelque chose dans la strategie doit changer. L'argent, le marche, l'offre — quelque chose ne fonctionne pas. Sois honnete avec toi-meme.

### Point de Controle de la Lecon 5

Tu devrais maintenant avoir :
- [ ] Un budget mensuel d'environ 200 $ reparti en quatre categories
- [ ] Des comptes API crees avec des limites de depenses configurees
- [ ] Des decisions d'infrastructure prises (local uniquement vs. local + VPS)
- [ ] Un stack d'outils selectionne (principalement des tiers gratuits au demarrage)
- [ ] Objectifs de ROI : 3x dans les 90 jours
- [ ] Une regle claire : augmenter le budget uniquement apres avoir prouve le ROI

*Dans le cours STREETS complet, le Module E (Execution Playbook) inclut un modele de tableau de bord financier qui suit tes depenses, revenus et ROI par moteur de revenus en temps reel — pour que tu saches toujours quels flux sont rentables et lesquels necessitent un ajustement.*

---

## Lecon 6 : Ton Document de Stack Souveraine

*"Toute activite a un plan. Voici le tien — et il tient en deux pages."*

### Le Livrable

C'est la chose la plus importante que tu creeras dans le Module S. Ton Document de Stack Souveraine est une reference unique qui capture tout sur ton infrastructure generatrice de revenus. Tu t'y refereras tout au long du reste du cours STREETS, tu le mettras a jour a mesure que ta configuration evolue, et tu l'utiliseras pour prendre des decisions lucides sur quoi construire et quoi ignorer.

Cree un nouveau fichier. Markdown, Google Doc, page Notion, texte brut — peu importe, tant que tu vas reellement le maintenir. Utilise le modele ci-dessous, en remplissant chaque champ avec les chiffres et decisions des Lecons 1 a 5.

### Le Modele

{? if computed.profile_completeness != "0" ?}
> **Longueur d'avance :** 4DA a deja detecte certaines de tes specs materiel et infos de stack. Cherche les indications pre-remplies ci-dessous — elles te feront gagner du temps pour remplir le modele.
{? endif ?}

Copie ce modele en entier et remplis-le. Chaque champ. Pas de raccourci.

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
> **Pre-remplissage a partir de ton Developer DNA :**
> - **Stack principal :** {= dna.primary_stack | fallback("Not detected") =}
> - **Interets :** {= dna.interests | fallback("Not detected") =}
> - **Resume d'identite :** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **Angles morts a surveiller :** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **Pre-remplissage a partir du stack detecte :** Tes technologies principales sont {= stack.primary | fallback("not yet detected") =}. {? if stack.adjacent ?}Competences adjacentes : {= stack.adjacent | fallback("none detected") =}.{? endif ?} Utilise-les pour remplir l'Inventaire des Competences ci-dessus.
{? endif ?}

{@ insight t_shape @}

### Comment Utiliser Ce Document

1. **Avant de commencer tout nouveau projet :** Consulte ta Stack Souveraine. As-tu le materiel, le temps, les competences et le budget pour executer ?
2. **Avant d'acheter quoi que ce soit :** Consulte ton allocation de budget. Cet achat est-il dans le plan ?
3. **Revue mensuelle :** Mets a jour la colonne "Reel" dans ton budget. Mets a jour les chiffres de revenus. Ajuste les allocations en fonction de ce qui marche.
4. **Quand quelqu'un te demande ce que tu fais :** Ta section "Ce que Je Peux Offrir Aujourd'hui" est ton pitch instantane.
5. **Quand tu es tente de courir apres une nouvelle idee brillante :** Consulte tes contraintes. Est-ce que ca rentre dans ton temps, tes competences et ton materiel ? Si non, ajoute-le a "Ce que Je Construis" pour plus tard.

### L'Exercice d'Une Heure

Mets un minuteur a 60 minutes. Remplis chaque champ du modele. Ne reflechis pas trop. Ne fais pas de recherche extensive. Ecris ce que tu sais maintenant. Tu pourras mettre a jour plus tard.

Les champs que tu ne peux pas remplir ? Ce sont tes actions a mener cette semaine :
- Chiffres de benchmark manquants ? Lance le script de benchmark de la Lecon 2.
- Pas d'entite juridique ? Lance le processus de creation de la Lecon 4.
- Pas de traitement des paiements ? Configure Stripe a partir de la Lecon 4.
- Inventaire de competences vide ? Passe 15 minutes a lister tout ce pour quoi on t'a paye au cours des 5 dernieres annees.

> **Erreur courante :** Passer 3 heures a rendre le document "parfait" au lieu d'1 heure a le rendre "fait." Le Document de Stack Souveraine est une reference de travail, pas un business plan pour des investisseurs. Personne ne le verra sauf toi. La precision compte. Le formatage non.

### Point de Controle de la Lecon 6

Tu devrais maintenant avoir :
- [ ] Un Document de Stack Souveraine complet sauvegarde quelque part ou tu vas reellement l'ouvrir
- [ ] Les six sections remplies avec des vrais chiffres (pas des chiffres aspirationnels)
- [ ] Une liste claire d'actions a mener pour les lacunes de ta configuration
- [ ] Une date fixee pour ta premiere revue mensuelle (dans 30 jours)

---

## Module S : Termine

{? if progress.completed("MODULE_S") ?}
> **Module S termine.** Tu as fini {= progress.completed_count | fallback("1") =} sur {= progress.total_count | fallback("7") =} modules STREETS. {? if progress.completed_modules ?}Termines : {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### Ce Que Tu As Construit en Deux Semaines

Regarde ce que tu as maintenant que tu n'avais pas quand tu as commence :

1. **Un inventaire materiel** cartographie selon les capacites generatrices de revenus — pas juste des specs sur un autocollant.
2. **Un stack LLM local de qualite production** avec Ollama, benchmarke sur ton materiel reel, configure pour de vraies charges de travail.
3. **Un avantage confidentialite** que tu sais comment marketer — avec un langage specifique pour des audiences specifiques.
4. **Une base juridique et financiere** — entite juridique (ou plan), traitement des paiements, compte bancaire, strategie fiscale.
5. **Un budget controle** avec des objectifs de ROI clairs et une echeance de 90 jours pour prouver le modele.
6. **Un Document de Stack Souveraine** qui capture tout ce qui precede dans une reference unique que tu utiliseras pour chaque decision a venir.

C'est plus que ce que la plupart des developpeurs mettent en place. Serieusement. La plupart des gens qui veulent generer des revenus secondaires sautent directement a "construire un truc cool" et se demandent ensuite pourquoi ils ne sont pas payes. Tu as maintenant l'infrastructure pour etre paye.

Mais une infrastructure sans direction n'est qu'un hobby couteux. Tu as besoin de savoir ou pointer ce stack.

{@ temporal market_timing @}

### La Suite : Module T — Avantages Techniques

Le Module S t'a donne les fondations. Le Module T repond a la question cruciale : **comment construire quelque chose que les concurrents ne peuvent pas facilement copier ?**

Voici ce que couvre le Module T :
- **Pipelines de donnees proprietaires** — comment creer des datasets auxquels toi seul as acces, legalement et ethiquement
- **Configurations de modeles personnalisees** — fine-tuning et prompt engineering qui produisent une qualite de sortie que les autres ne peuvent pas atteindre avec les parametres par defaut
- **Stacks de competences a rendement compose** — pourquoi "Python + sante" bat "Python + JavaScript" pour les revenus, et comment identifier ta combinaison unique
- **Barrieres techniques a l'entree** — des designs d'infrastructure qui prendraient des mois a un concurrent pour les repliquer
- **L'Audit de Fosse** — un framework pour evaluer si ton projet a un avantage defensif ou est juste un service commodity de plus

La difference entre un developpeur qui gagne 500 $/mois et un qui gagne 5 000 $/mois est rarement la competence. Ce sont les fosses. Des choses qui rendent ton offre difficile a repliquer, meme si quelqu'un a le meme materiel et les memes modeles.

### La Feuille de Route STREETS Complete

| Module | Titre | Focus | Duree |
|--------|-------|-------|----------|
| **S** | Configuration Souveraine | Infrastructure, juridique, budget | Semaines 1-2 (termine) |
| **T** | Avantages Techniques | Avantages defensifs, actifs proprietaires | Semaines 3-4 |
| **R** | Moteurs de Revenus | Playbooks de monetisation specifiques avec code | Semaines 5-8 |
| **E** | Execution Playbook | Sequences de lancement, tarification, premiers clients | Semaines 9-10 |
| **E** | Avantage Evolutif | Garder une longueur d'avance, detection de tendances, adaptation | Semaines 11-12 |
| **T** | Automatisation Tactique | Automatiser les operations pour des revenus passifs | Semaines 13-14 |
| **S** | Empiler les Flux | Sources de revenus multiples, strategie de portefeuille | Semaines 15-16 |

Le Module R (Moteurs de Revenus) est la ou la plupart de l'argent se fait. Mais sans S et T, tu construis sur du sable.

---

**Pret pour le playbook complet ?**

Tu as vu les fondations. Tu les as construites toi-meme. Maintenant, obtiens le systeme complet.

**Obtiens STREETS Core** — le cours complet de 16 semaines avec les sept modules, les templates de code pour les moteurs de revenus, les tableaux de bord financiers, et la communaute privee de developpeurs qui construisent des revenus a leurs propres conditions.

*Ta machine. Tes regles. Tes revenus.*
