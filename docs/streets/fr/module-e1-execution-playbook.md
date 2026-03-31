# Module E : Manuel d'Execution

**Cours STREETS de Revenus pour Developpeurs — Module Payant**
*Semaines 9-10 | 6 Lecons | Livrable : Ton Premier Produit, En Ligne et Acceptant les Paiements*

> "De l'idee au deploiement en 48 heures. Sans trop reflechir."

---

Tu as l'infrastructure (Module S). Tu as le fosse defensif (Module T). Tu as les conceptions de moteur de revenus (Module R). Maintenant il est temps de livrer.

Ce module est celui que la plupart des developpeurs n'atteignent jamais — non pas parce qu'il est difficile, mais parce qu'ils sont encore en train de polir leur base de code, de refactoriser leur architecture, d'ajuster leur palette de couleurs. Ils font tout sauf la chose qui compte : mettre un produit devant un etre humain qui peut payer pour ca.

Livrer est une competence. Comme toute competence, elle devient plus facile avec la pratique et pire avec le retard. Plus tu attends, plus ca devient difficile. Plus tu livres, moins ca fait peur. Ton premier lancement sera desordonné. C'est le but.

A la fin de ces deux semaines, tu auras :

- Une idee de produit validee testee contre des signaux de demande reels
- Un produit en ligne, deploye et accessible via un vrai domaine
- Un traitement des paiements acceptant de l'argent reel
- Au moins un lancement public sur une plateforme ou se rassemble ton audience cible
- Un systeme de metriques post-lancement pour guider tes prochains pas

Pas d'hypothetiques. Pas de "en theorie." Un vrai produit, en ligne sur internet, capable de generer des revenus.

{? if progress.completed("R") ?}
Tu as termine le Module R — tu as deja des conceptions de moteur de revenus pretes a executer. Ce module transforme l'une de ces conceptions en un produit en ligne.
{? else ?}
Si tu n'as pas encore termine le Module R, tu peux quand meme utiliser ce module — mais avoir une conception de moteur de revenus prete rendra le sprint de 48 heures nettement plus fluide.
{? endif ?}

{@ mirror execution_readiness @}

Construisons-le.

---

## Lecon 1 : Le Sprint de 48 Heures

*"Samedi matin a dimanche soir. Un produit. Zero excuses."*

### Pourquoi 48 Heures

La Loi de Parkinson dit que le travail s'etend pour remplir le temps disponible. Donne-toi 6 mois pour construire un produit et tu passeras 5 mois a deliberer et 1 mois dans une frenesie stressce. Donne-toi 48 heures et tu prendras des decisions, reduiras le scope sans pitie et livreras quelque chose de reel.

La contrainte de 48 heures ne consiste pas a construire quelque chose de parfait. Il s'agit de construire quelque chose qui existe. L'existence bat la perfection a chaque fois, parce qu'un produit en ligne genere des donnees — qui visite, qui clique, qui paie, qui se plaint — et les donnees te disent quoi construire ensuite.

Chaque produit developpeur reussi que j'ai etudie a suivi ce schema : livre vite, apprends vite, itere vite. Ceux qui ont echoue ? Ils ont tous de beaux fichiers README et zero utilisateurs.

Voici ton manuel minute par minute.

### Jour 1 — Samedi

#### Bloc du Matin (4 heures) : Valider la Demande

Avant d'ecrire une seule ligne de code, tu as besoin de preuves que quelqu'un d'autre que toi veut cette chose. Pas de certitude — des preuves. La difference compte. La certitude est impossible. Les preuves sont realisables en 4 heures.

**Etape 1 : Verification du Volume de Recherche (45 minutes)**

Va sur ces sources et cherche ton idee de produit et les termes associes :

- **Google Trends** (https://trends.google.com) — Gratuit. Montre l'interet de recherche relatif dans le temps. Tu veux voir une ligne plate ou montante, pas une descendante.
- **Ahrefs Free Webmaster Tools** (https://ahrefs.com/webmaster-tools) — Gratuit avec verification du site. Montre les volumes de mots-cles.
- **Ubersuggest** (https://neilpatel.com/ubersuggest/) — Le tier gratuit donne 3 recherches/jour. Montre le volume de recherche, la difficulte et les termes associes.
- **AlsoAsked** (https://alsoasked.com) — Tier gratuit. Montre les donnees "Autres questions posees" de Google. Revele quelles questions les gens posent reellement.

Ce que tu cherches :

```
BONS signaux :
- 500+ recherches mensuelles pour ton mot-cle principal
- Tendance montante sur les 12 derniers mois
- Plusieurs questions "Autres questions posees" sans bonnes reponses
- Mots-cles long-tail associes avec faible concurrence

MAUVAIS signaux :
- Interet de recherche en baisse
- Zero volume de recherche (personne ne cherche ca)
- Domine par des entreprises massives en page 1
- Pas de variation dans les termes de recherche (trop etroit)
```

Exemple reel : Supposons que ton idee de moteur de revenus du Module R est une "bibliotheque de composants Tailwind CSS pour les dashboards SaaS."

```
Recherche : "tailwind dashboard components" — 2 900/mois, tendance montante
Recherche : "tailwind admin template" — 6 600/mois, stable
Recherche : "react dashboard template tailwind" — 1 300/mois, montant
Associe : "shadcn dashboard", "tailwind analytics components"

Verdict : Demande forte. Plusieurs angles de mots-cles. Continuer.
```

Autre exemple : Supposons que ton idee est un "anonymiseur de fichiers log base sur Rust."

```
Recherche : "log file anonymizer" — 90/mois, plat
Recherche : "anonymize log files" — 140/mois, plat
Recherche : "PII removal from logs" — 320/mois, montant
Associe : "GDPR log compliance", "scrub PII from logs"

Verdict : Niche mais en croissance. L'angle "PII removal" a plus de volume
que l'angle "anonymizer". Reformule ton positionnement.
```

**Etape 2 : Extraction de Fils de Discussion Communautaires (60 minutes)**

Va la ou les developpeurs demandent des choses et cherche dans ton espace problematique :

- **Reddit :** Cherche dans r/webdev, r/reactjs, r/selfhosted, r/SideProject, r/programming et les subreddits de niche pertinents pour ton domaine
- **Hacker News :** Utilise https://hn.algolia.com pour chercher des discussions passees
- **GitHub Issues :** Cherche des issues dans les repos populaires lies a ton espace
- **Stack Overflow :** Cherche des questions avec beaucoup de votes mais des reponses acceptees insatisfaisantes
- **Serveurs Discord :** Verifie les serveurs de communautes developpeurs pertinents

Ce que tu documentes :

```markdown
## Resultats de l'Extraction de Fils

### Fil 1
- **Source :** Reddit r/reactjs
- **URL :** [lien]
- **Titre :** "Is there a good Tailwind dashboard kit that isn't $200?"
- **Votes positifs :** 147
- **Commentaires :** 83
- **Citations cles :**
  - "Everything on the market is either free and ugly, or $200+ and overkill"
  - "I just need 10-15 well-designed components, not 500"
  - "Would pay $49 for something that actually looks good out of the box"
- **Conclusion :** Sensibilite au prix a $200+, volonte de payer a $29-49

### Fil 2
- ...
```

Trouve au moins 5 fils. Si tu ne peux pas trouver 5 fils ou les gens demandent quelque chose dans l'espace de ton produit, c'est un signal d'alerte serieux. Soit la demande n'existe pas, soit tu cherches avec les mauvais termes. Essaie differents mots-cles avant d'abandonner l'idee.

**Etape 3 : Audit des Concurrents (45 minutes)**

Cherche ce qui existe deja. Ce n'est pas decourageant — c'est validant. Des concurrents signifient qu'il y a un marche. Pas de concurrents signifie generalement qu'il n'y a pas de marche, pas que tu as trouve un ocean bleu.

Pour chaque concurrent, documente :

```markdown
## Audit des Concurrents

### Concurrent 1 : [Nom]
- **URL :** [lien]
- **Prix :** $XX
- **Ce qu'ils font bien :** [choses specifiques]
- **Ce qui est nul :** [plaintes specifiques des avis/fils]
- **Leurs avis :** [verifie G2, avis ProductHunt, mentions Reddit]
- **Ton angle :** [comment tu ferais differemment]

### Concurrent 2 : [Nom]
- ...
```

L'or est dans "ce qui est nul." Chaque plainte sur un concurrent est une demande de fonctionnalite pour ton produit. Les gens te disent litteralement quoi construire et combien facturer.

**Etape 4 : Le Test "10 Personnes Paieraient" (30 minutes)**

C'est la porte de validation finale. Tu dois trouver des preuves qu'au moins 10 personnes paieraient de l'argent pour ca. Pas "exprime de l'interet." Pas "dit que c'etait cool." Paieraient.

Sources de preuves :
- Fils Reddit ou les gens disent "Je paierais pour X" (signal le plus fort)
- Produits concurrents avec des clients payants (prouve que le marche paie)
- Produits Gumroad/Lemon Squeezy dans ton espace avec des compteurs de ventes visibles
- Repos GitHub avec 1 000+ etoiles qui resolvent un probleme associe (les gens valorisent suffisamment pour mettre une etoile)
- Ta propre audience si tu en as une (tweete, envoie un DM a 10 personnes, demande directement)

Si tu passes ce test : continue. Construis-le.

Si tu echoues a ce test : pivote ton angle, pas toute ton idee. La demande pourrait exister dans un espace adjacent. Essaie un positionnement different avant d'abandonner.

> **Parlons Franchement :** La plupart des developpeurs sautent completement la validation parce qu'ils veulent coder. Ils passeront 200 heures a construire quelque chose que personne n'a demande, puis se demanderont pourquoi personne n'achete. Ces 4 heures de recherche te sauveront 196 heures d'effort gaspille. Ne saute pas cette etape. Le code est la partie facile.

#### Bloc de l'Apres-midi (4 heures) : Construire le MVP

Tu as valide la demande. Tu as la recherche sur les concurrents. Tu sais ce que les gens veulent et ce qui manque aux solutions existantes. Maintenant construis la version minimale qui resout le probleme central.

{? if profile.gpu.exists ?}
Avec un GPU dans ta machine ({= profile.gpu.model | fallback("ton GPU") =}), considere des idees de produit qui exploitent l'inference IA locale — outils de traitement d'images, utilitaires d'analyse de code, pipelines de generation de contenu. Les fonctionnalites alimentees par GPU sont un vrai differenciateur que la plupart des developpeurs independants ne peuvent pas offrir.
{? endif ?}

**La Regle des 3 Fonctionnalites**

Ton v0.1 a exactement 3 fonctionnalites. Pas 4. Pas 7. Trois.

Comment les choisir :
1. Quelle est l'UNIQUE chose que fait ton produit ? (Fonctionnalite 1 — le noyau)
2. Qu'est-ce qui le rend utilisable ? (Fonctionnalite 2 — generalement l'authentification, ou sauvegarder/exporter, ou la configuration)
3. Qu'est-ce qui le rend digne d'etre paye par rapport aux alternatives ? (Fonctionnalite 3 — ton differenciateur)

Tout le reste va sur une liste "v0.2" que tu ne touches pas ce week-end.

Exemple reel — une bibliotheque de composants dashboard Tailwind :
1. **Noyau :** 12 composants dashboard prets pour la production (graphiques, tableaux, cartes de statistiques, navigation)
2. **Utilisable :** Extraits de code a copier-coller avec apercu en direct
3. **Differenciateur :** Mode sombre integre, composants concus pour fonctionner ensemble (pas une collection aleatoire)

Exemple reel — un outil CLI de nettoyage PII dans les logs :
1. **Noyau :** Detecter et masquer les PII des fichiers log (emails, IPs, noms, SSNs)
2. **Utilisable :** Fonctionne comme un pipe CLI (`cat logs.txt | pii-scrub > clean.txt`)
3. **Differenciateur :** Fichier de regles configurable, gere 15+ formats de log automatiquement

{@ insight stack_fit @}

**Preparer le Projet**

Utilise les LLMs pour accelerer, pas remplacer, ton travail. Voici le workflow pratique :

{? if stack.contains("react") ?}
Puisque ton stack principal inclut React, le scaffold d'app web ci-dessous est ton chemin le plus rapide. Tu connais deja l'outillage — concentre tes 48 heures sur la logique du produit, pas sur l'apprentissage d'un nouveau framework.
{? elif stack.contains("rust") ?}
Puisque ton stack principal inclut Rust, le scaffold d'outil CLI ci-dessous est ton chemin le plus rapide. Les outils CLI Rust ont une excellente distribution (binaire unique, multiplateforme) et les audiences developpeurs respectent l'histoire de performance.
{? elif stack.contains("python") ?}
Puisque ton stack principal inclut Python, considere un outil CLI ou un service API. Python livre vite avec FastAPI ou Typer, et l'ecosysteme PyPI te donne une distribution instantanee a des millions de developpeurs.
{? endif ?}

```bash
# Scaffold d'une app web (outil SaaS, bibliotheque de composants avec site de docs, etc.)
pnpm create vite@latest my-product -- --template react-ts
cd my-product
pnpm install

# Ajouter Tailwind CSS (le plus courant pour les produits developpeurs)
pnpm install -D tailwindcss @tailwindcss/vite

# Ajouter le routage si tu as besoin de plusieurs pages
pnpm install react-router-dom

# Structure du projet — garde-la plate pour un build de 48 heures
mkdir -p src/components src/pages src/lib
```

```bash
# Scaffold d'un outil CLI (pour les utilitaires developpeurs)
cargo init my-tool
cd my-tool

# Dependances courantes pour les outils CLI
cargo add clap --features derive    # Parsing des arguments
cargo add serde --features derive   # Serialisation
cargo add serde_json                # Gestion du JSON
cargo add anyhow                    # Gestion des erreurs
cargo add regex                     # Correspondance de motifs
```

```bash
# Scaffold d'un paquet npm (pour les bibliotheques/utilitaires)
mkdir my-package && cd my-package
pnpm init
pnpm install -D typescript tsup vitest
mkdir src
```

**Le Workflow LLM pour Construire**

{? if settings.has_llm ?}
Tu as un LLM configure ({= settings.llm_provider | fallback("local") =} / {= settings.llm_model | fallback("ton modele") =}). Utilise-le comme ton programmeur en binome pendant le sprint — il accelere considerablement le scaffolding et la generation de boilerplate.
{? endif ?}

Ne demande pas au LLM de construire tout ton produit. Ca produit du code generique et fragile. A la place :

1. **Toi** tu ecris l'architecture : structure de fichiers, flux de donnees, interfaces cles
2. **LLM** genere le boilerplate : composants repetitifs, fonctions utilitaires, definitions de types
3. **Toi** tu ecris la logique centrale : la partie qui rend ton produit different
4. **LLM** genere les tests : tests unitaires, cas limites, tests d'integration
5. **Toi** tu revois et edites tout : ton nom est sur ce produit

Travail parallele pendant que tu codes : ouvre un second chat LLM et fais-lui rediger le texte de ta landing page, le README et la documentation. Tu les editeras le soir, mais les premiers brouillons seront prets.

**Discipline du Temps**

```
14:00 — Fonctionnalite 1 (fonctionnalite centrale) : 2 heures
          Si ca ne fonctionne pas a 16:00, reduis le scope.
16:00 — Fonctionnalite 2 (utilisabilite) : 1 heure
          Reste simple. Le polissage viendra plus tard.
17:00 — Fonctionnalite 3 (differenciateur) : 1 heure
          C'est ce qui te rend digne d'etre paye. Concentre-toi ici.
18:00 — ARRETE DE CODER. Ca n'a pas besoin d'etre parfait.
```

> **Erreur Courante :** "Juste une fonctionnalite de plus avant d'arreter." C'est comme ca que les projets de week-end deviennent des projets d'un mois. Les 3 fonctionnalites sont ton scope. Si tu penses a une super idee pendant la construction, ecris-la sur ta liste v0.2 et continue. Tu pourras l'ajouter la semaine prochaine apres avoir des clients payants.

#### Bloc du Soir (2 heures) : Ecrire la Landing Page

Ta landing page a un seul boulot : convaincre un visiteur de payer. Elle n'a pas besoin d'etre belle. Elle doit etre claire.

**La Landing Page en 5 Sections**

Chaque landing page reussie de produit developpeur suit cette structure. Ne la reinvente pas :

```
Section 1 : TITRE + SOUS-TITRE
  - Ce que ca fait en 8 mots ou moins
  - Pour qui c'est et quel resultat ils obtiennent

Section 2 : LE PROBLEME
  - 3 points de douleur que ton client cible reconnait
  - Utilise leur langage exact de ton extraction de fils

Section 3 : LA SOLUTION
  - Captures d'ecran ou exemples de code de ton produit
  - 3 fonctionnalites liees aux 3 points de douleur ci-dessus

Section 4 : TARIFS
  - Un ou deux niveaux. Garde ca simple pour la v0.1.
  - Option de facturation annuelle si c'est un abonnement.

Section 5 : CTA (Appel a l'Action)
  - Un bouton. "Commencer", "Acheter", "Telecharger".
  - Repete le benefice central.
```

**Exemple Reel de Texte — Kit Dashboard Tailwind :**

```markdown
# Section 1
## DashKit — Composants Dashboard Tailwind Prets pour la Production
Livre ton dashboard SaaS en heures, pas en semaines.
12 composants copier-coller. Mode sombre. $29.

# Section 2
## Le Probleme
- Les kits UI generiques te donnent 500 composants mais zero cohesion
- Construire des UIs de dashboard de zero prend 40+ heures
- Les options gratuites ressemblent a du Bootstrap de 2018

# Section 3
## Ce Que Tu Obtiens
- **12 composants** concus pour fonctionner ensemble (pas une collection aleatoire)
- **Mode sombre** integre — bascule avec un prop
- **Code copier-coller** — pas de npm install, pas de dependances, pas de lock-in
[capture d'ecran d'exemples de composants]

# Section 4
## Tarifs
**DashKit** — $29 paiement unique
- Les 12 composants avec code source
- Mises a jour gratuites pendant 12 mois
- Utilisation dans des projets illimites

**DashKit Pro** — $59 paiement unique
- Tout dans DashKit
- 8 modeles de pages completes (analytics, CRM, admin, parametres)
- Fichiers design Figma
- Demandes de fonctionnalites prioritaires

# Section 5
## Livre ton dashboard ce week-end.
[Acheter DashKit — $29]
```

**Exemple Reel de Texte — Nettoyeur PII de Logs :**

```markdown
# Section 1
## ScrubLog — Supprime les PII des Fichiers Log en Secondes
Conformite RGPD pour tes logs. Une commande.

# Section 2
## Le Probleme
- Tes logs contiennent des emails, IPs et noms que tu ne devrais pas stocker
- La redaction manuelle prend des heures et rate des choses
- Les outils entreprise coutent $500/mois et necessitent un doctorat pour les configurer

# Section 3
## Comment Ca Fonctionne
```bash
cat server.log | scrublog > clean.log
```
- Detecte 15+ motifs PII automatiquement
- Regles personnalisees via configuration YAML
- Gere les formats JSON, Apache, Nginx et texte brut
[capture terminal montrant avant/apres]

# Section 4
## Tarifs
**Personnel** — Gratuit
- 5 motifs PII, 1 format de log

**Pro** — $19/mois
- Tous les 15+ motifs PII
- Tous les formats de log
- Regles personnalisees
- Partage de configuration d'equipe

# Section 5
## Arrete de stocker des PII dont tu n'as pas besoin.
[Obtenir ScrubLog Pro — $19/mois]
```

**Workflow LLM pour le Texte :**

1. Nourris le LLM avec ton audit de concurrents et les resultats d'extraction de fils
2. Demande-lui de rediger le texte de la landing page en utilisant le modele 5 sections
3. Edite sans pitie : remplace chaque phrase vague par une specifique
4. Lis-le a haute voix. Si une phrase te fait grimacer, reecris-la.

**Construire la Landing Page :**

Pour un sprint de 48 heures, ne construis pas une landing page personnalisee de zero. Utilise l'une de celles-ci :

{? if stack.contains("react") ?}
- **Ton app React** — Puisque tu travailles en React, fais de la landing page la page d'accueil deconnectee de ton app ou ajoute une route marketing dans Next.js. Zero cout de changement de contexte.
{? endif ?}
- **Le propre site de ton produit** — Si c'est une app web, fais de la landing page la page d'accueil deconnectee
- **Astro + Tailwind** — Site statique, deploiement sur Vercel en 2 minutes, extremement rapide
- **Next.js** — Si ton produit est deja React, ajoute une route de page marketing
- **Framer** (https://framer.com) — Constructeur visuel, exporte du code propre, tier gratuit disponible
- **Carrd** (https://carrd.co) — $19/an, sites d'une page ultra simples

```bash
# Le chemin le plus rapide : site statique Astro
pnpm create astro@latest my-product-site
cd my-product-site
pnpm install
# Ajouter Tailwind
pnpm astro add tailwind
```

Tu devrais avoir une landing page avec du texte a la fin du samedi. Elle n'a pas besoin d'illustrations personnalisees. Elle n'a pas besoin d'animations. Elle a besoin de mots clairs et d'un bouton d'achat.

### Jour 2 — Dimanche

#### Bloc du Matin (3 heures) : Deployer

Ton produit doit etre en ligne sur internet a une vraie URL. Pas localhost. Pas une URL de preview Vercel avec un hash aleatoire. Un vrai domaine, avec HTTPS, que tu peux partager et que les gens peuvent visiter.

**Etape 1 : Deployer l'Application (60 minutes)**

{? if computed.os_family == "windows" ?}
Puisque tu es sur Windows, assure-toi que WSL2 est disponible si tes outils de deploiement le necessitent. La plupart des outils CLI de deploiement (Vercel, Fly.io) fonctionnent nativement sur Windows, mais certains scripts supposent des chemins Unix.
{? elif computed.os_family == "macos" ?}
Sur macOS, tous les CLIs de deploiement s'installent proprement via Homebrew ou telechargement direct. Tu es sur le chemin de deploiement le plus fluide.
{? elif computed.os_family == "linux" ?}
Sur Linux, tu as l'environnement de deploiement le plus flexible. Tous les outils CLI fonctionnent nativement, et tu peux aussi auto-heberger sur ta propre machine si tu as une IP statique et veux economiser sur les couts d'hebergement.
{? endif ?}

Choisis ta plateforme de deploiement selon ce que tu as construit :

**Site statique / SPA (bibliotheque de composants, landing page, site de docs) :**
```bash
# Vercel — le chemin le plus rapide pour les sites statiques et Next.js
pnpm install -g vercel
vercel

# Il te posera des questions. Dis oui a tout.
# Ton site est en ligne en ~60 secondes.
```

**App web avec backend (outil SaaS, service API) :**
```bash
# Railway — simple, bon tier gratuit, gere les bases de donnees
# https://railway.app
# Connecte ton repo GitHub et deploie.

# Ou Fly.io — plus de controle, deploiement edge global
# https://fly.io
curl -L https://fly.io/install.sh | sh
fly launch
fly deploy
```

**Outil CLI / paquet npm :**
```bash
# registre npm
npm publish

# Ou distribue comme binaire via GitHub Releases
# Utilise cargo-dist pour les projets Rust
cargo install cargo-dist
cargo dist init
cargo dist build
# Telecharge les binaires vers la release GitHub
```

**Etape 2 : Acheter un Domaine (30 minutes)**

Un vrai domaine coute $12/an. Si tu ne peux pas investir $12 dans ton business, tu ne prends pas ton business au serieux.

**Ou acheter :**
- **Namecheap** (https://namecheap.com) — $8-12/an pour .com, bonne gestion DNS
- **Cloudflare Registrar** (https://dash.cloudflare.com) — Prix au cout (souvent $9-10/an pour .com), excellent DNS
- **Porkbun** (https://porkbun.com) — Souvent le moins cher la premiere annee, bonne interface

**Conseils pour nommer les domaines :**
- Plus court est mieux. 2 syllabes ideal, 3 max.
- `.com` gagne toujours en confiance. `.dev` et `.io` sont bien pour les outils developpeurs.
- Verifie la disponibilite chez ton registraire, pas chez GoDaddy (ils font du front-running de recherches).
- Ne passe pas plus de 15 minutes a choisir. Le nom compte moins que tu ne le penses.

```bash
# Pointe ton domaine vers Vercel
# Dans le dashboard Vercel : Settings > Domains > Add your domain
# Puis dans les parametres DNS de ton registraire, ajoute :
# A record: @ -> 76.76.21.21
# CNAME record: www -> cname.vercel-dns.com

# Ou si tu utilises Cloudflare pour le DNS :
# Ajoute simplement les memes enregistrements dans le panneau DNS Cloudflare
# SSL est automatique avec Vercel et Cloudflare
```

**Etape 3 : Monitoring de Base (30 minutes)**

Tu dois savoir deux choses : est-ce que le site est en ligne, et est-ce que des gens le visitent.

**Monitoring d'uptime (gratuit) :**
- **Better Uptime** (https://betteruptime.com) — Le tier gratuit surveille 10 URLs toutes les 3 minutes
- **UptimeRobot** (https://uptimerobot.com) — Le tier gratuit surveille 50 URLs toutes les 5 minutes

```
Configure le monitoring pour :
1. L'URL de ta landing page
2. L'endpoint de sante de ton app (si applicable)
3. L'URL de ton webhook de paiement (critique — tu dois savoir si les paiements cassent)
```

**Analytics (respectueux de la vie privee) :**

N'utilise pas Google Analytics. Ton audience developpeur le bloque, c'est excessif pour un nouveau produit, et c'est un risque pour la vie privee.

- **Plausible** (https://plausible.io) — $9/mois, vie privee d'abord, script d'une seule ligne
- **Fathom** (https://usefathom.com) — $14/mois, vie privee d'abord, leger
- **Umami** (https://umami.is) — Gratuit et auto-heberge, ou $9/mois cloud

```html
<!-- Plausible — une ligne dans ton <head> -->
<script defer data-domain="yourdomain.com"
  src="https://plausible.io/js/script.js"></script>

<!-- Umami — une ligne dans ton <head> -->
<script defer
  src="https://your-umami-instance.com/script.js"
  data-website-id="your-website-id"></script>
```

> **Parlons Franchement :** Oui, $9/mois pour des analytics sur un produit qui n'a pas encore gagne d'argent semble inutile. Mais tu ne peux pas ameliorer ce que tu ne peux pas mesurer. Le premier mois de donnees analytics te dira plus sur ton marche qu'un mois de devinettes. Si $9/mois casse ton budget, auto-heberge Umami gratuitement sur Railway.

#### Bloc de l'Apres-midi (2 heures) : Configurer les Paiements

Si ton produit ne peut pas accepter d'argent, c'est un projet hobby. Configurer les paiements prend moins de temps que la plupart des developpeurs ne le pensent — environ 20-30 minutes pour le flux basique.

{? if regional.country ?}
> **Processeurs de paiement recommandes pour {= regional.country | fallback("ton pays") =} :** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy, PayPal") =}. Les options ci-dessous sont disponibles mondialement, mais verifie que ton processeur prefere supporte les paiements en {= regional.currency | fallback("ta monnaie locale") =}.
{? endif ?}

**Option A : Lemon Squeezy (Recommande pour les Produits Numeriques)**

Lemon Squeezy (https://lemonsqueezy.com) gere le traitement des paiements, la taxe de vente, la TVA et la livraison numerique sur une seule plateforme. C'est le chemin le plus rapide de zero a accepter des paiements.

Pourquoi Lemon Squeezy plutot que Stripe pour ton premier produit :
- Agit comme Merchant of Record — ils gerent la taxe de vente, la TVA et la conformite pour toi
- Pages de checkout integrees — pas de travail frontend necessaire
- Livraison numerique integree — telecharge tes fichiers, ils gerent l'acces
- 5% + $0,50 par transaction (plus eleve que Stripe, mais t'economise des heures de maux de tete fiscaux)

Guide de configuration :
1. Inscris-toi sur https://app.lemonsqueezy.com
2. Cree une Boutique (le nom de ton business)
3. Ajoute un Produit :
   - Nom, description, prix
   - Telecharge les fichiers pour la livraison numerique (si applicable)
   - Configure les cles de licence (si tu vends du logiciel)
4. Obtiens ton URL de checkout — c'est vers quoi ton bouton "Acheter" pointe
5. Configure un webhook pour l'automatisation post-achat

```javascript
// Gestionnaire webhook Lemon Squeezy (Node.js/Express)
// POST /api/webhooks/lemonsqueezy

import crypto from 'crypto';

const WEBHOOK_SECRET = process.env.LEMONSQUEEZY_WEBHOOK_SECRET;

export async function handleLemonSqueezyWebhook(req, res) {
  // Verifier la signature du webhook
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

      // Envoyer email de bienvenue, accorder l'acces, creer cle de licence, etc.
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

**Option B : Stripe (Plus de Controle, Plus de Travail)**

Stripe (https://stripe.com) te donne plus de controle mais necessite que tu geres la conformite fiscale separement. Mieux pour le SaaS avec une facturation complexe.

```javascript
// Session Stripe Checkout (Node.js)
// Cree une page de checkout hebergee

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
          unit_amount: 5900, // $59,00 en centimes
        },
        quantity: 1,
      },
    ],
    mode: 'payment', // 'subscription' pour recurrent
    success_url: `${process.env.DOMAIN}/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${process.env.DOMAIN}/pricing`,
    customer_email: req.body.email, // Pre-remplir si tu l'as
  });

  return res.json({ url: session.url });
}

// Gestionnaire webhook Stripe
export async function handleStripeWebhook(req, res) {
  const sig = req.headers['stripe-signature'];

  let event;
  try {
    event = stripe.webhooks.constructEvent(
      req.body, // corps brut, pas du JSON parse
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

**Pour les Deux Plateformes — Teste Avant de Lancer :**

```bash
# Lemon Squeezy : Utilise le mode test dans le dashboard
# Active "Test mode" en haut a droite du dashboard Lemon Squeezy
# Utilise le numero de carte : 4242 4242 4242 4242, n'importe quelle date future, n'importe quel CVC

# Stripe : Utilise les cles API du mode test
# Carte de test : 4242 4242 4242 4242
# Carte de test qui decline : 4000 0000 0000 0002
# Carte de test necessitant l'auth : 4000 0025 0000 3155
```

Parcours tout le flux d'achat toi-meme en mode test. Clique le bouton d'achat, termine le checkout, verifie que le webhook se declenche, verifie que l'acces est accorde. Si une etape echoue en mode test, elle echouera pour les vrais clients.

> **Erreur Courante :** "Je configurerai les paiements plus tard, quand j'aurai des utilisateurs." C'est a l'envers. Configurer les paiements ne consiste pas a collecter de l'argent aujourd'hui — c'est valider si quelqu'un paiera. Un produit sans prix est un outil gratuit. Un produit avec un prix est un test de business. Le prix lui-meme fait partie de la validation.

#### Bloc du Soir (3 heures) : Lancer

Ton produit est en ligne. Les paiements fonctionnent. La landing page est claire. Maintenant tu as besoin que des humains le voient.

**La Strategie de Lancement Doux**

Ne fais pas un "grand lancement" pour ton premier produit. Les grands lancements creent de la pression pour etre parfait, et ton v0.1 n'est pas parfait. A la place, fais un lancement doux : partage-le a quelques endroits, recueille du feedback, corrige les problemes critiques, puis fais le grand lancement dans 1-2 semaines.

**Plateforme de Lancement 1 : Reddit (30 minutes)**

Publie dans r/SideProject et un subreddit de niche pertinent pour ton produit.

Modele de publication Reddit :

```markdown
Title: I built [ce que ca fait] in a weekend — [benefice cle]

Body:
Hey [subreddit],

I've been frustrated with [le probleme] for a while, so I built
[nom du produit] this weekend.

**What it does:**
- [Fonctionnalite 1 — la valeur centrale]
- [Fonctionnalite 2]
- [Fonctionnalite 3]

**What makes it different from [concurrent] :**
[Un paragraphe honnete sur ton differenciateur]

**Pricing:**
[Sois transparent. "$29 one-time" ou "Free tier + $19/mo Pro"]

I'd love feedback. What am I missing? What would make this
useful for your workflow?

[Lien vers le produit]
```

Regles pour les publications Reddit :
- Sois genuinement utile, pas vendeur
- Reponds a chaque commentaire individuel (ce n'est pas optionnel)
- Accepte les critiques avec grace — le feedback negatif est le plus precieux
- Ne fais pas d'astroturfing (faux votes, comptes multiples). Tu seras pris et banni.

**Plateforme de Lancement 2 : Hacker News (30 minutes)**

Si ton produit est technique et interessant, publie un Show HN. Dans la section "Details techniques", mentionne ton stack ({= stack.primary | fallback("ton stack principal") =}) et explique pourquoi tu l'as choisi — les lecteurs de HN adorent les decisions techniques informees.

Modele Show HN :

```markdown
Title: Show HN: [Nom du Produit] – [ce que ca fait en <70 caracteres]

Body:
[Nom du produit] is [une phrase expliquant ce que ca fait].

I built this because [motivation genuine — quel probleme tu resolvais
pour toi-meme].

Technical details:
- Built with [stack]
- [Decision technique interessante et pourquoi]
- [Ce qui rend l'implementation remarquable]

Try it: [URL]

Feedback welcome. I'm particularly interested in [question specifique pour
l'audience HN].
```

Conseils HN :
- Publie entre 7-9h du matin heure de l'Est US (plus fort trafic)
- Le titre compte plus que tout. Sois specifique et technique.
- Les lecteurs HN respectent la substance technique plutot que le marketing poli
- Reponds aux commentaires immediatement dans les 2 premieres heures. La velocite des commentaires affecte le classement.
- Ne supplie pas pour des votes. Publie et interagis.

**Plateforme de Lancement 3 : Twitter/X (30 minutes)**

Ecris un fil de lancement build-in-public :

```
Tweet 1 (Accroche) :
I built [produit] in 48 hours this weekend.

It [resout un probleme specifique] for [audience specifique].

Here's what I shipped, what I learned, and the real numbers. Thread:

Tweet 2 (Le Probleme) :
The problem:
[Decris le point de douleur en 2-3 phrases]
[Inclus une capture d'ecran ou un exemple de code montrant la douleur]

Tweet 3 (La Solution) :
So I built [nom du produit].

[Capture d'ecran/GIF du produit en action]

It does three things:
1. [Fonctionnalite 1]
2. [Fonctionnalite 2]
3. [Fonctionnalite 3]

Tweet 4 (Detail Technique) :
Tech stack for the nerds:
- [Frontend]
- [Backend]
- [Hosting — mentionne la plateforme specifique]
- [Paiements — mentionne Lemon Squeezy/Stripe]
- Total cost to run: $XX/month

Tweet 5 (Tarifs) :
Pricing:
[Tarifs clairs, comme sur la landing page]
[Lien vers le produit]

Tweet 6 (Demande) :
Would love feedback from anyone who [decris l'utilisateur cible].

What am I missing? What would make this a must-have for you?
```

**Plateforme de Lancement 4 : Communautes Pertinentes (30 minutes)**

Identifie 2-3 communautes ou ton audience cible passe du temps :

- Serveurs Discord (communautes developpeurs, serveurs specifiques de frameworks)
- Communautes Slack (beaucoup de communautes dev de niche ont des groupes Slack)
- Dev.to / Hashnode (ecris un court post "J'ai construit ca")
- Indie Hackers (https://indiehackers.com) — specifiquement concu pour ca
- Groupes Telegram ou WhatsApp pertinents

**Premieres 48 Heures Apres le Lancement — Quoi Surveiller :**

```
Metriques a suivre :
1. Visiteurs uniques (depuis les analytics)
2. Taux de clic landing page -> checkout (devrait etre 2-5%)
3. Taux de conversion checkout -> achat (devrait etre 1-3%)
4. Taux de rebond (au-dessus de 80% signifie que ton titre/hero est mauvais)
5. Sources de trafic (d'ou viennent tes visiteurs ?)
6. Commentaires et feedback (qualitatif — que disent les gens ?)

Calcul exemple :
- 500 visiteurs en 48 heures (raisonnable depuis Reddit + HN + Twitter)
- 3% cliquent "Acheter" = 15 visites checkout
- 10% completent l'achat = 1-2 ventes
- A $29/vente = $29-58 ton premier week-end

Ce n'est pas de l'argent pour la retraite. C'est de l'argent de VALIDATION.
$29 d'un inconnu sur internet prouve que ton produit a de la valeur.
```

Ne panique pas si tu obtiens zero vente dans les premieres 48 heures. Regarde ton entonnoir :
- Zero visiteurs ? Ta distribution est le probleme, pas ton produit.
- Des visiteurs mais zero clics sur "Acheter" ? Ton texte ou ton prix est le probleme.
- Des clics sur "Acheter" mais zero achats completes ? Ton flux de checkout est casse ou ton prix est trop eleve pour la valeur percue.

Chacun de ceux-ci a une solution differente. C'est pourquoi les metriques comptent.

### A Ton Tour

1. **Bloque le temps.** Ouvre ton calendrier maintenant et bloque le prochain samedi de 8h a 20h et dimanche de 8h a 20h. Nomme-le "Sprint de 48 Heures." Traite-le comme un vol que tu ne peux pas reporter.

2. **Choisis ton idee.** Choisis un moteur de revenus du Module R. Ecris le scope de 3 fonctionnalites pour ton v0.1. Si tu ne peux pas en choisir un, prends celui que tu pourrais expliquer a un non-developpeur en une phrase.
{? if dna.primary_stack ?}
   Ton chemin d'execution le plus fort est de construire quelque chose avec {= dna.primary_stack | fallback("ton stack principal") =} — livre plus vite la ou tu as deja une expertise profonde.
{? endif ?}

3. **Pre-travail.** Avant samedi, cree des comptes sur :
   - Vercel, Railway ou Fly.io (deploiement)
   - Lemon Squeezy ou Stripe (paiements)
   - Namecheap, Cloudflare ou Porkbun (domaine)
   - Plausible, Fathom ou Umami (analytics)
   - Better Uptime ou UptimeRobot (monitoring)

   Fais ca un soir de semaine pour que le samedi soit du pur code, pas de la creation de comptes.

4. **Prepare tes plateformes de lancement.** Si tu n'as pas de compte Reddit avec un peu de karma, commence a participer dans les subreddits pertinents cette semaine. Les comptes qui ne publient que de l'auto-promotion sont signales. Si tu n'as pas de compte Hacker News, crees-en un et participe a quelques discussions d'abord.

---

## Lecon 2 : L'Etat d'Esprit "Livre, Puis Ameliore"

*"v0.1 avec 3 fonctionnalites bat v1.0 qui ne sort jamais."*

### Le Piege du Perfectionnisme

Les developpeurs sont particulierement susceptibles a un mode d'echec specifique : construire en prive indefiniment. Nous savons a quoi ressemble du "bon code." Nous savons que notre v0.1 n'est pas du bon code. Alors nous refactorisons. Nous ajoutons la gestion des erreurs. Nous ecrivons plus de tests. Nous ameliorons l'architecture. Nous faisons tout sauf la seule chose qui compte : le montrer a des humains.

Voici une verite qui te sauvera des milliers d'heures : **tes clients ne lisent pas ton code source.** Ils ne se soucient pas de ton architecture. Ils ne se soucient pas de ta couverture de tests. Ils se soucient d'une chose : est-ce que ca resout mon probleme ?

Un produit avec du code spaghetti qui resout un vrai probleme gagnera de l'argent. Un produit avec une belle architecture qui ne resout aucun probleme ne gagnera rien.

Ce n'est pas une excuse pour ecrire du mauvais code. C'est une declaration de priorites. Livre d'abord. Refactorise ensuite. Le refactoring sera de toute facon mieux informe par les donnees d'utilisation reelles.

### Comment "Livre, Puis Ameliore" Se Deroule

Considere ce scenario : un developpeur lance un pack de templates Notion pour les managers en ingenierie logicielle. Voici a quoi ca ressemble au lancement :

- 5 templates (pas 50)
- Une page Gumroad avec un paragraphe de description et 3 captures d'ecran
- Pas de site web personnalise
- Pas de liste email
- Pas de suiveurs sur les reseaux sociaux
- Prix : $29

Ils le publient sur Reddit et Twitter. C'est toute la strategie marketing.

Resultats du Mois 1 :
- ~170 ventes a $29 = ~$5 000
- Apres la commission de Gumroad (10%) : ~$4 500
- Temps investi : ~30 heures au total (construire les templates + ecrire les descriptions)
- Taux horaire effectif : ~$150/heure

C'etait "parfait" ? Non. Les templates avaient des incoherences de formatage. Certaines descriptions etaient generiques. Les clients s'en fichaient. Ils se souciaient que ca leur evitait de construire les templates eux-memes.

Au mois 3, base sur les retours clients, le developpeur :
- A corrige les problemes de formatage
- A ajoute plus de templates (ceux que les clients avaient specifiquement demandes)
- A augmente le prix a $39 (les clients existants ont recu les mises a jour gratuitement)
- A cree un niveau "Pro" avec un video walkthrough accompagnant

Le produit qu'ils ont lance etait pire a tous egards que le produit qu'ils avaient 90 jours plus tard. Mais la version des 90 jours n'existait que parce que la version de lancement avait genere le feedback et les revenus pour guider le developpement.

> **NOTE :** Pour une validation du monde reel du modele "lance moche, ameliore vite" : Josh Comeau a pre-vendu $550K de son cours CSS pour Developpeurs JavaScript la premiere semaine (Source : failory.com). Wes Bos a genere $10M+ de ventes totales de cours developpeurs en utilisant des lancements iteratifs (Source : foundershut.com). Les deux ont commence avec des produits v1 imparfaits et ont itere base sur du vrai feedback client.

### Les 10 Premiers Clients Te Disent Tout

Tes 10 premiers clients payants sont les personnes les plus importantes de ton business. Pas a cause de leur argent — 10 ventes a $29 c'est $290, ce qui t'achete des courses. Ils sont importants parce qu'ils sont des volontaires pour ton equipe de developpement produit.

Que faire avec tes 10 premiers clients :

1. **Envoie un email de remerciement personnel.** Pas automatise. Personnel. "Salut, j'ai vu que tu as achete [produit]. Merci. Je developpe ca activement — y a-t-il quelque chose que tu souhaiterais que ca fasse et que ca ne fait pas ?"

2. **Lis chaque reponse.** Certains ne repondront pas. Certains repondront avec "ca a l'air bien, merci." Mais 2-3 sur 10 ecriront des paragraphes sur ce qu'ils veulent. Ces paragraphes sont ta feuille de route.

3. **Cherche des patterns.** Si 3 sur 10 personnes demandent la meme fonctionnalite, construis-la. C'est un signal de demande de 30% de clients payants. Aucun sondage ne te donnera des donnees aussi bonnes.

4. **Demande leur volonte de payer plus.** "Je prevois un niveau Pro avec [fonctionnalite X]. Est-ce que ca vaudrait $49 pour toi ?" Direct. Specifique. Te donne des donnees de tarification.

```
Modele d'email pour les 10 premiers clients :

Objet : Petite question sur [nom du produit]

Salut [nom],

J'ai vu que tu as pris [nom du produit] — merci d'etre
l'un des premiers clients.

Je construis ca activement et livre des mises a jour chaque
semaine. Petite question : quelle est l'UNIQUE chose que tu
souhaiterais que ca fasse et que ca ne fait pas ?

Il n'y a pas de mauvaises reponses. Meme si ca semble etre
une grosse demande, je veux l'entendre.

Merci,
[Ton nom]
```

### Comment Gerer le Feedback Negatif

Ton premier feedback negatif semblera personnel. Ce n'est pas personnel. Ce sont des donnees.

**Framework pour traiter le feedback negatif :**

```
1. PAUSE. Ne reponds pas pendant 30 minutes. Ta reaction emotionnelle
   n'est pas utile.

2. CATEGORISE le feedback :
   a) Rapport de bug — corrige-le. Remercie-les.
   b) Demande de fonctionnalite — ajoute au backlog. Remercie-les.
   c) Plainte sur le prix — note-la. Verifie si c'est un pattern.
   d) Plainte sur la qualite — investigue. Est-elle valide ?
   e) Troll/deraisonnable — ignore. Passe a autre chose.

3. REPONDS (pour a, b, c, d seulement) :
   "Merci pour le feedback. [Reconnais le probleme specifique].
   Je [le corrige maintenant / l'ajoute a la feuille de route / examine ca].
   Je te tiendrai au courant quand c'est resolu."

4. AGIS. Si tu as promis de corriger quelque chose, corrige-le dans la semaine.
   Rien ne construit la loyaute plus vite que de montrer aux clients que
   leur feedback mene a de vrais changements.
```

> **Parlons Franchement :** Quelqu'un te dira que ton produit est nul. Ca fera mal. Mais si ton produit est en ligne et gagne de l'argent, tu as deja fait quelque chose que la plupart des developpeurs ne font jamais. La personne qui critique depuis la section commentaires n'a rien livre. Toi si. Continue a livrer.

### Le Cycle d'Iteration Hebdomadaire

Apres le lancement, ton workflow devient une boucle serree :

```
Lundi :     Revise les metriques de la semaine derniere et le feedback client
Mardi :     Planifie l'amelioration de cette semaine (UNE chose, pas cinq)
Mercredi :  Construis l'amelioration
Jeudi :     Teste et deploie l'amelioration
Vendredi :  Ecris un post changelog/mise a jour
Week-end :  Marketing — un post de blog, un post social, une interaction communautaire

Repete.
```

Le mot cle est UNE amelioration par semaine. Pas une refonte de fonctionnalites. Pas un redesign. Une chose qui rend le produit legerement meilleur pour tes clients existants. Sur 12 semaines, ca fait 12 ameliorations guidees par des donnees d'utilisation reelles. Ton produit apres 12 semaines de ce cycle sera radicalement meilleur que tout ce que tu aurais pu concevoir en isolation.

### Les Revenus Valident Plus Vite Que les Sondages

Les sondages mentent. Pas intentionnellement — les gens sont juste mauvais pour predire leur propre comportement. "Paierais-tu $29 pour ca ?" obtient des reponses faciles de "oui." Mais "voici la page de checkout, entre ta carte de credit" obtient des reponses honnetes.

C'est pourquoi tu lances avec des paiements des le jour un :

| Methode de Validation | Temps pour le Signal | Qualite du Signal |
|---|---|---|
| Sondage / poll | 1-2 semaines | Faible (les gens mentent) |
| Landing page avec inscription email | 1-2 semaines | Moyenne (interet, pas engagement) |
| Landing page avec prix mais sans checkout | 1 semaine | Moyenne-Haute (acceptation du prix) |
| **Produit en ligne avec vrai checkout** | **48 heures** | **La plus haute (comportement d'achat reel)** |

Le prix de $0 ne revele rien. Le prix de $29 revele tout.

### A Ton Tour

1. **Ecris ton engagement de "lancement moche."** Ouvre un fichier texte et ecris : "Je lancerai [nom du produit] le [date] meme si ce n'est pas parfait. Scope v0.1 : [3 fonctionnalites]. Je n'ajouterai pas la Fonctionnalite 4 avant le lancement." Signe-le (metaphoriquement). Consulte-le quand l'envie de polir frappe.

2. **Redige ton email pour les 10 premiers clients.** Ecris le modele d'email de remerciement personnel maintenant, avant d'avoir des clients. Quand la premiere vente arrive, tu veux l'envoyer dans l'heure.

3. **Configure ton tracker d'iterations.** Cree un simple tableur ou une page Notion avec les colonnes : Semaine | Amelioration Realisee | Impact sur les Metriques | Feedback Client. Ca devient ton journal de decisions pour savoir quoi construire ensuite.

---

## Lecon 3 : Psychologie des Prix pour les Produits Developpeurs

*"$0 n'est pas un prix. C'est un piege."*

### Pourquoi le Gratuit Coute Cher

La verite la plus contre-intuitive pour vendre des produits developpeurs : **les utilisateurs gratuits te coutent plus que les clients payants.**

Utilisateurs gratuits :
- Envoient plus de demandes de support (ils n'ont rien a perdre)
- Exigent plus de fonctionnalites (ils se sentent en droit parce qu'ils ne paient pas)
- Fournissent un feedback moins utile ("c'est cool" n'est pas actionnable)
- Partent a des taux plus eleves (il n'y a pas de cout de changement)
- Parlent moins de ton produit (les choses gratuites ont une faible valeur percue)

Clients payants :
- Sont investis dans ton succes (ils veulent que leur achat soit une bonne decision)
- Fournissent un feedback specifique et actionnable (ils veulent que le produit s'ameliore)
- Sont plus faciles a retenir (ils ont deja decide de payer ; l'inertie joue en ta faveur)
- Recommandent plus souvent (recommander quelque chose pour lequel on a paye valide l'achat)
- Respectent ton temps (ils comprennent que tu geres un business)

La seule raison d'offrir un tier gratuit est comme mecanisme de generation de leads pour le tier payant. Si ton tier gratuit est assez bon pour que les gens n'upgradent jamais, tu n'as pas un tier gratuit — tu as un produit gratuit avec un bouton de don.

> **Erreur Courante :** "Je le rendrai gratuit pour avoir des utilisateurs d'abord, puis je facturerai plus tard." Ca ne marche presque jamais. Les utilisateurs que tu attires a $0 s'attendent a $0 pour toujours. Quand tu ajoutes un prix, ils partent. Les utilisateurs qui auraient paye $29 des le premier jour n'ont jamais trouve ton produit parce que tu l'as positionne comme un outil gratuit. Tu as attire la mauvaise audience.

{@ insight cost_projection @}

### Les Niveaux de Prix pour les Produits Developpeurs

Apres avoir analyse des centaines de produits developpeurs reussis, ces points de prix fonctionnent de maniere coherente. Tous les prix ci-dessous sont en USD — si tu tarifies en {= regional.currency | fallback("ta monnaie locale") =}, ajuste pour le pouvoir d'achat local et les normes du marche.

**Niveau 1 : $9-29 — Outils et Utilitaires Developpeurs**

Les produits dans cette gamme resolvent un probleme specifique et etroit. Un seul achat, utilise-le aujourd'hui.

```
Exemples :
- Extension VS Code avec fonctionnalites premium : $9-15
- Outil CLI avec fonctionnalites pro : $15-19
- Outil SaaS a usage unique : $9-19/mois
- Petite bibliotheque de composants : $19-29
- Extension DevTools navigateur : $9-15

Psychologie de l'acheteur : Territoire d'achat impulsif. Le developpeur le voit,
reconnait le probleme, l'achete sans demander a son manager.
Pas d'approbation de budget necessaire. Carte de credit -> termine.

Insight cle : A ce prix, ta landing page doit convertir en
moins de 2 minutes. L'acheteur ne lira pas une longue liste de fonctionnalites.
Montre le probleme, montre la solution, montre le prix.
```

**Niveau 2 : $49-99 — Templates, Kits et Outils Complets**

Les produits dans cette gamme economisent un temps significatif. Plusieurs composants travaillant ensemble.

```
Exemples :
- Kit complet de templates UI : $49-79
- Boilerplate SaaS avec auth, facturation, dashboards : $79-99
- Ensemble complet d'icones/illustrations : $49-69
- Toolkit CLI multi-usage : $49
- Bibliotheque wrapper API avec docs extensifs : $49-79

Psychologie de l'acheteur : Achat reflechi. Le developpeur evalue
pendant 5-10 minutes. Compare aux alternatives. Calcule le temps economise.
"Si ca me sauve 10 heures et je valorise mon temps a $50/heure,
$79 est une evidence."

Insight cle : Tu as besoin d'un point de comparaison. Montre le temps/effort
necessaire pour construire ca de zero vs. acheter ton kit.
Inclus des temoignages si tu en as.
```

**Niveau 3 : $149-499 — Cours, Solutions Completes, Templates Premium**

Les produits dans cette gamme transforment une competence ou fournissent un systeme complet.

```
Exemples :
- Cours video (10+ heures) : $149-299
- Kit de demarrage SaaS avec source complete + video walkthrough : $199-299
- Bibliotheque de composants entreprise : $299-499
- Toolkit developpeur complet (outils multiples) : $199
- Codebase complet + lecons "Construis X De Zero" : $149-249

Psychologie de l'acheteur : Achat d'investissement. L'acheteur doit justifier
la depense (a lui-meme ou son manager). Ils ont besoin de preuve
sociale, d'apercus detailles et d'un recit ROI clair.

Insight cle : A ce niveau, offre une garantie de remboursement.
Ca reduit l'anxiete d'achat et augmente les conversions. Les taux de
remboursement pour les produits numeriques developpeurs sont typiquement 3-5%.
L'augmentation des conversions depasse largement les remboursements.
```

### La Strategie de Tarification a 3 Niveaux

Si ton produit le supporte, offre trois niveaux de prix. Ce n'est pas aleatoire — ca exploite un biais cognitif bien documente appele l'"effet de scene centrale." Quand trois options sont presentees, la plupart des gens choisissent celle du milieu.

```
Structure des niveaux :

BASIQUE         PRO (mis en avant)     EQUIPE/ENTREPRISE
$29             $59                    $149
Fonctionnalites Tout dans Basique      Tout dans Pro
de base          + fonctionnalites     + fonctionnalites equipe
                 premium               + licence commerciale
                 + support prioritaire

Distribution de conversion (typique) :
- Basique : 20-30%
- Pro : 50-60% <- c'est ta cible
- Equipe : 10-20%
```

**Comment concevoir les niveaux :**

1. Commence par le niveau **Pro**. C'est le produit que tu veux reellement vendre, au prix qui reflete sa valeur. Concois-le en premier.

2. Cree le niveau **Basique** en enlevant des fonctionnalites de Pro. Enleve suffisamment pour que Basique resolve le probleme mais Pro le resolve *bien*. Basique devrait etre legerement frustrant — utilisable, mais clairement limite.

3. Cree le niveau **Equipe** en ajoutant des fonctionnalites a Pro. Licences multi-sieges, droits d'utilisation commerciale, support prioritaire, branding personnalise, acces au code source, fichiers Figma, etc.

**Exemple reel de page de tarifs :**

```
DashKit

STARTER — $29                    PRO — $59                        EQUIPE — $149
                                 * Le Plus Populaire               Ideal pour les agences

* 12 composants de base          * Tout dans Starter               * Tout dans Pro
* React + TypeScript              * 8 modeles pages completes       * Jusqu'a 5 membres d'equipe
* Mode sombre                     * Fichiers design Figma           * Licence commerciale
* npm install                     * Table de donnees avancee          (projets clients illimites)
* 6 mois de mises a jour         * Integration bibliotheque         * Support prioritaire
                                   de graphiques                    * Mises a jour a vie
                                 * 12 mois de mises a jour          * Options de branding perso
                                 * Demandes de fonctionnalites
                                   prioritaires

[Obtenir Starter]                [Obtenir Pro]                     [Obtenir Equipe]
```

### Ancrage des Prix

L'ancrage est le biais cognitif ou le premier nombre que les gens voient influence leur perception des nombres suivants. Utilise-le ethiquement :

1. **Montre l'option chere en premier** (a droite dans les layouts occidentaux). Voir $149 fait paraitre $59 raisonnable.

2. **Montre des calculs d'"heures economisees".**
   ```
   "Construire ces composants de zero prend ~40 heures.
   A $50/heure, ca fait $2 000 de ton temps.
   DashKit Pro : $59."
   ```

3. **Utilise la reformulation "par jour" pour les abonnements.**
   ```
   "$19/mois" -> "Moins de $0,63/jour"
   "$99/an" -> "$8,25/mois" ou "$0,27/jour"
   ```

4. **Reduction facturation annuelle.** Offre 2 mois gratuits sur les plans annuels. C'est standard et attendu. La facturation annuelle reduit le taux de depart de 30-40% parce que l'annulation necessite une decision consciente a un seul point de renouvellement, pas une decision mensuelle continue.

```
Mensuel : $19/mois
Annuel : $190/an (economise $38 — 2 mois gratuits)

Affiche comme :
Mensuel : $19/mois
Annuel : $15,83/mois (facture annuellement a $190)
```

### Tests A/B des Prix

Tester les prix est precieux mais delicat. Voici comment le faire sans etre malhonnete :

**Approches acceptables :**
- Teste differents prix sur differents canaux de lancement (Reddit obtient $29, Product Hunt obtient $39, regarde lequel convertit mieux)
- Change ton prix apres 2 semaines et compare les taux de conversion
- Offre une reduction de lancement ("$29 cette semaine, $39 apres") et regarde si l'urgence change le comportement
- Teste differentes structures de niveaux (2 niveaux vs 3 niveaux) sur differentes periodes

**Non acceptable :**
- Montrer differents prix a differents visiteurs sur la meme page en meme temps (discrimination par les prix, erode la confiance)
- Facturer plus base sur la localisation ou la detection du navigateur (les gens parlent, et tu seras pris)

### Quand Augmenter les Prix

Augmente tes prix quand l'une de ces conditions est vraie :

1. **Le taux de conversion est au-dessus de 5%.** Tu es trop bon marche. Un taux de conversion sain pour une landing page de produit developpeur est 1-3%. Au-dessus de 5% signifie que presque tout le monde qui voit le prix est d'accord que c'est une bonne affaire — ce qui signifie que tu laisses de l'argent sur la table.

2. **Personne ne s'est plaint du prix.** Si zero personne sur 100 dit que c'est trop cher, c'est trop bon marche. Un produit sain a environ 20% des visiteurs qui pensent que le prix est eleve. Ca signifie que 80% pensent que c'est juste ou une bonne affaire.

3. **Tu as ajoute des fonctionnalites significatives depuis le lancement.** Tu as lance a $29 avec 3 fonctionnalites. Maintenant tu as 8 fonctionnalites et une meilleure documentation. Le produit vaut plus. Facture plus.

4. **Tu as des temoignages et de la preuve sociale.** La valeur percue augmente avec la preuve sociale. Une fois que tu as 5+ avis positifs, ton produit vaut plus dans l'esprit de l'acheteur.

**Comment augmenter les prix :**
- Annonce l'augmentation de prix 1-2 semaines a l'avance ("Le prix passe de $29 a $39 le [date]")
- Les clients existants conservent l'ancien prix
- Ce n'est pas louche — c'est une pratique standard et ca cree aussi de l'urgence pour les indecis

> **Parlons Franchement :** La plupart des developpeurs sous-tarifent de 50-200%. Ton produit a {= regional.currency_symbol | fallback("$") =}29 vaut probablement {= regional.currency_symbol | fallback("$") =}49. Ton produit a {= regional.currency_symbol | fallback("$") =}49 vaut probablement {= regional.currency_symbol | fallback("$") =}79. Je le sais parce que les developpeurs s'ancrent a leur propre volonte de payer (faible — nous sommes radins avec l'outillage) plutot qu'a la volonte de payer du client (plus elevee — ils achetent une solution a un probleme qui leur coute du temps). Augmente tes prix plus tot que tu ne le penses.

### A Ton Tour

1. **Tarifie ton produit.** Base sur l'analyse des niveaux ci-dessus, choisis un point de prix pour ton lancement v0.1. Ecris-le. Si tu te sens mal a l'aise parce que ca semble "trop cher," tu es probablement dans la bonne fourchette. Si ca semble confortable, ajoute 50%.

2. **Concois ta page de tarifs.** En utilisant le modele a 3 niveaux, concois le texte de ta page de tarifs. Identifie quelles fonctionnalites vont dans chaque niveau. Identifie ton niveau "mis en avant" (celui que tu veux que la plupart des gens achetent).

3. **Calcule tes chiffres.** Complete :
   - Prix par vente : {= regional.currency_symbol | fallback("$") =}___
   - Revenu mensuel cible : {= regional.currency_symbol | fallback("$") =}___
   - Nombre de ventes necessaires par mois : ___
   - Visiteurs de landing page estimes necessaires (a 2% de conversion) : ___
   - Ce nombre de visiteurs est-il atteignable avec ton plan de distribution ? (Oui/Non)

---

## Lecon 4 : Configuration Juridique Minimum Viable

*"30 minutes de configuration juridique maintenant t'economisent 30 heures de panique plus tard."*

### La Verite Honnete Sur la Configuration Juridique

La plupart des developpeurs ignorent completement le juridique (risque) ou se paralysent avec (gaspillage). La bonne approche est une configuration juridique minimum viable : assez de protection pour operer legitimement, sans depenser $5 000 chez un avocat avant d'avoir gagne $5.

Voici ce dont tu as reellement besoin avant ta premiere vente, ce dont tu as besoin avant ta 100e vente, et ce dont tu n'as pas besoin avant bien plus tard.

### Avant Ta Premiere Vente (Fais Ca Ce Week-end)

**1. Verifie Ton Contrat de Travail (30 minutes)**

Si tu as un emploi a temps plein, lis la clause PI de ton contrat de travail avant de construire quoi que ce soit. Cherche specifiquement :

- **Clauses de cession d'inventions :** Certains contrats disent que tout ce que tu crees pendant ton emploi — y compris sur ton temps libre — appartient a ton employeur.
- **Clauses de non-concurrence :** Certaines te limitent a travailler dans la meme industrie, meme comme projet parallele.
- **Politiques de travail secondaire :** Certaines exigent une approbation ecrite pour les activites commerciales externes.

```
Ce que tu cherches :

SUR : "Les inventions faites pendant le temps de l'entreprise ou
utilisant les ressources de l'entreprise appartiennent a l'entreprise."
-> Ton projet de week-end sur ta machine personnelle est a toi.

FLOU : "Toutes les inventions liees aux affaires actuelles ou
prevues de l'entreprise." -> Si ton projet parallele est dans le
meme domaine que ton employeur, cherche un avis juridique.

RESTRICTIF : "Toutes les inventions concues pendant la periode
d'emploi appartiennent a l'entreprise." -> C'est agressif mais
courant dans certaines entreprises. Cherche un avis juridique avant de continuer.
```

Des etats comme la Californie, le Delaware, l'Illinois, le Minnesota, Washington et d'autres ont des lois qui limitent la portee avec laquelle les employeurs peuvent revendiquer tes inventions personnelles. Mais le langage specifique de ton contrat compte.

> **Erreur Courante :** "Je le garderai secret." Si ton produit devient suffisamment reussi pour compter, quelqu'un le remarquera. Si ca viole ton contrat de travail, tu pourrais perdre le produit ET ton emploi. 30 minutes de lecture de ton contrat maintenant empechent ca.

**2. Politique de Confidentialite (15 minutes)**

Si ton produit collecte des donnees — meme juste une adresse email pour l'achat — tu as besoin d'une politique de confidentialite. C'est une exigence legale dans l'UE (RGPD), en Californie (CCPA), et de plus en plus partout ailleurs.

N'en ecris pas une de zero. Utilise un generateur :

- **Termly** (https://termly.io/products/privacy-policy-generator/) — Tier gratuit, reponds a des questions, obtiens une politique
- **Avodocs** (https://www.avodocs.com) — Gratuit, modeles juridiques open-source
- **Iubenda** (https://www.iubenda.com) — Tier gratuit, genere automatiquement base sur ton stack tech

Ta politique de confidentialite doit couvrir :

```markdown
# Politique de Confidentialite pour [Nom du Produit]
Derniere mise a jour : [Date]

## Ce Que Nous Collectons
- Adresse email (pour la confirmation d'achat et les mises a jour produit)
- Informations de paiement (traitees par [Lemon Squeezy/Stripe],
  nous ne voyons ni ne stockons jamais les details de ta carte)
- Analytics d'utilisation basiques (pages vues, utilisation des fonctionnalites — via
  [Plausible/Fathom/Umami], respectueux de la vie privee, sans cookies)

## Ce Que Nous Ne Collectons PAS
- Nous ne te suivons pas sur le web
- Nous ne vendons tes donnees a personne
- Nous n'utilisons pas de cookies publicitaires

## Comment Nous Utilisons Tes Donnees
- Pour livrer le produit que tu as achete
- Pour envoyer les mises a jour produit et les avis importants
- Pour ameliorer le produit base sur les patterns d'utilisation agreges

## Stockage des Donnees
- Tes donnees sont stockees sur les serveurs de [fournisseur d'hebergement] dans [region]
- Les donnees de paiement sont entierement gerees par [Lemon Squeezy/Stripe]

## Tes Droits
- Tu peux demander une copie de tes donnees a tout moment
- Tu peux demander la suppression de tes donnees a tout moment
- Contact : [ton email]

## Modifications
- Nous te notifierons des changements significatifs par email
```

Mets ca a `tondomaine.com/privacy`. Lie-le depuis le pied de page de ta page de checkout.

**3. Conditions d'Utilisation (15 minutes)**

Tes conditions d'utilisation te protegent contre les reclamations deraisonnables. Pour un produit numerique, elles sont simples.

```markdown
# Conditions d'Utilisation pour [Nom du Produit]
Derniere mise a jour : [Date]

## Licence
Quand tu achetes [Nom du Produit], tu recois une licence pour l'utiliser
a des fins [personnelles/commerciales].

- **Licence individuelle :** Utilisation dans tes propres projets (illimites)
- **Licence equipe :** Utilisation par jusqu'a [N] membres d'equipe
- Tu ne peux PAS redistribuer, revendre ou partager les identifiants d'acces

## Remboursements
- Produits numeriques : garantie de remboursement de [30 jours / 14 jours]
- Si tu n'es pas satisfait, envoie un email a [ton email] pour un remboursement complet
- Aucune question dans la periode de remboursement

## Responsabilite
- [Nom du Produit] est fourni "tel quel" sans garantie
- Nous ne sommes pas responsables des dommages decoulant de l'utilisation du produit
- La responsabilite maximale est limitee au montant que tu as paye

## Support
- Le support est fourni par email a [ton email]
- Nous visons a repondre dans les [48 heures / 2 jours ouvrables]

## Modifications
- Nous pouvons mettre a jour ces conditions avec preavis
- L'utilisation continue constitue l'acceptation des conditions mises a jour
```

Mets ca a `tondomaine.com/terms`. Lie-le depuis le pied de page de ta page de checkout.

### Avant Ta 100e Vente (Premiers Mois)

**4. Entite Commerciale (1-3 heures + temps de traitement)**

Operer en tant qu'entrepreneur individuel (le defaut quand tu vends des choses sans creer une entreprise) fonctionne pour tes premieres ventes. Mais quand les revenus augmentent, tu veux une protection de responsabilite et des avantages fiscaux.

{? if regional.country ?}
> **Pour {= regional.country | fallback("ta region") =} :** Le type d'entite recommande est une **{= regional.business_entity_type | fallback("LLC ou equivalent") =}**, avec des couts d'enregistrement typiques de {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Trouve la section de ton pays ci-dessous pour des conseils specifiques.
{? endif ?}

**Etats-Unis — LLC :**

Une LLC (Limited Liability Company) est le choix standard pour les business de developpeurs solo.

```
Cout : $50-500 selon l'etat (frais de depot)
Temps : 1-4 semaines de traitement
Ou deposer : Ton etat de residence, sauf s'il y a une raison specifique
d'utiliser le Delaware ou le Wyoming

Depot DIY (moins cher) :
1. Va sur le site web du Secretary of State de ton etat
2. Depose les "Articles of Organization" (le formulaire fait generalement 1-2 pages)
3. Paie les frais de depot ($50-250 selon l'etat)
4. Obtiens ton EIN (identifiant fiscal) sur IRS.gov — gratuit, instantane en ligne

Comparaison des etats pour developpeurs solo :
- Wyoming : $100 depot, $60/an rapport annuel. Pas d'impot sur le revenu d'etat.
             Bon pour la vie privee (pas d'info membre publique requise).
- Delaware : $90 depot, $300/an taxe annuelle. Populaire mais pas
            necessairement mieux pour les developpeurs solo.
- New Mexico : $50 depot, pas de rapport annuel. Le moins cher a maintenir.
- California : $70 depot, $800/an taxe de franchise minimum.
              Cher. Tu paies ca meme si tu gagnes $0.
```

**Stripe Atlas (si tu veux que ce soit fait pour toi) :**

Stripe Atlas (https://atlas.stripe.com) coute $500 et configure une LLC du Delaware, un compte bancaire US (via Mercury), un compte Stripe, et fournit des guides fiscaux et juridiques. Si tu n'es pas aux US ou si tu veux juste que quelqu'un d'autre gere la paperasse, ca vaut les $500.

**Royaume-Uni — Ltd Company :**

```
Cout : GBP 12 a Companies House (https://www.gov.uk/set-up-limited-company)
Temps : Generalement 24-48 heures
Continu : Declaration de confirmation annuelle (GBP 13), depot des comptes annuels

Pour les developpeurs solo : Une Ltd company te donne une protection
de responsabilite et une efficacite fiscale une fois que les benefices
depassent ~GBP 50 000/an. En dessous, sole trader est plus simple.
```

**Union Europeenne :**

Chaque pays a sa propre structure. Options courantes :
- **Allemagne :** GmbH (chere a creer) ou enregistrement freelance (peu cher)
- **Pays-Bas :** BV ou eenmanszaak (entreprise individuelle)
- **France :** Auto-entrepreneur (micro-entreprise) — tres courant pour les developpeurs solo, impot forfaitaire simple
- **Estonie :** E-Residency + OUe estonienne (populaire chez les nomades numeriques, entreprise EU complete pour ~EUR 190)

**Australie :**

```
Sole trader : Gratuit a enregistrer via demande ABN (https://www.abr.gov.au)
Company (Pty Ltd) : AUD 538 enregistrement aupres de l'ASIC
Pour les developpeurs solo : Commence en sole trader. Enregistre une entreprise
quand les revenus justifient la charge comptable (~AUD 100K+/an).
```

**5. Obligations Fiscales**

Si tu utilises Lemon Squeezy comme plateforme de paiement, ils gerent la taxe de vente et la TVA en tant que Merchant of Record. C'est une simplification massive.

Si tu utilises Stripe directement, tu es responsable de :
- **Taxe de vente US :** Varie par etat. Utilise Stripe Tax ($0,50/transaction) ou TaxJar pour automatiser.
- **TVA UE :** 20-27% selon le pays. Requis pour les ventes numeriques aux clients UE quel que soit ton lieu. Lemon Squeezy gere ca ; Stripe Tax peut l'automatiser.
- **TVA UK :** 20%. Requis si tes ventes UK depassent GBP 85 000/an.
- **Taxes sur les Services Numeriques :** Divers pays les imposent. Une autre raison d'utiliser Lemon Squeezy jusqu'a ce que ton volume justifie de gerer ca toi-meme.

{? if regional.country ?}
> **Note fiscale pour {= regional.country | fallback("ta region") =} :** {= regional.tax_note | fallback("Consulte un professionnel fiscal local pour les details de tes obligations.") =}
{? endif ?}

> **Parlons Franchement :** Le plus grand avantage de Lemon Squeezy sur Stripe pour un developpeur solo n'est pas la page de checkout ou les fonctionnalites. C'est qu'ils gerent la conformite fiscale a l'echelle mondiale. La taxe de vente internationale est un cauchemar. Lemon Squeezy prend 5% + $0,50 par transaction et fait disparaitre le cauchemar. Jusqu'a ce que tu gagnes {= regional.currency_symbol | fallback("$") =}5 000+/mois, les 5% en valent la peine. Apres ca, evalue si gerer les impots toi-meme avec Stripe + TaxJar t'economise de l'argent et du stress.

**6. Bases de la Propriete Intellectuelle**

Ce que tu dois savoir :

- **Ton code est automatiquement protege par le droit d'auteur** au moment ou tu l'ecris. Pas de depot necessaire. Mais le depot (US : $65 a copyright.gov) te donne une position juridique plus forte en cas de litige.
- **Le nom de ton produit peut etre une marque deposee.** Pas requis pour le lancement, mais considere-le si le produit decolle. Depot de marque US : $250-350 par classe.
- **Les licences open-source dans tes dependances comptent.** Si tu utilises du code sous licence MIT, tout va bien. Si tu utilises du code sous licence GPL dans un produit commercial, tu pourrais devoir rendre ton produit open-source. Verifie les licences de tes dependances avant de vendre.

```bash
# Verifie les licences des dependances de ton projet (Node.js)
npx license-checker --summary

# Verifie specifiquement les licences problematiques
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"

# Pour les projets Rust
cargo install cargo-license
cargo license
```

**7. Assurance**

Tu n'as pas besoin d'assurance pour une bibliotheque de composants a $29. Tu as besoin d'assurance si :
- Tu fournis des services (conseil, traitement de donnees) ou les erreurs pourraient causer des pertes aux clients
- Ton produit gere des donnees sensibles (sante, finance)
- Tu signes des contrats avec des clients entreprise (ils l'exigeront)

Quand tu en as besoin, l'assurance responsabilite professionnelle (erreurs et omissions / E&O) coute $500-1 500/an pour un business de developpeur solo.

### A Ton Tour

1. **Lis ton contrat de travail.** Si tu es employe, trouve la clause PI et la clause de non-concurrence. Categorise-les : Sur / Flou / Restrictif. Si Flou ou Restrictif, consulte un avocat en droit du travail avant de lancer (beaucoup offrent des consultations gratuites de 30 minutes).

2. **Genere tes documents juridiques.** Va sur Termly ou Avodocs et genere une politique de confidentialite et des conditions d'utilisation pour ton produit. Sauvegarde-les en HTML ou Markdown. Deploie-les a `/privacy` et `/terms` sur le domaine de ton produit.

3. **Prends ta decision d'entite.** Base sur les conseils ci-dessus et ta residence en {= regional.country | fallback("ton pays") =}, decide : lancer en tant qu'entrepreneur individuel (le plus rapide) ou creer une {= regional.business_entity_type | fallback("LLC/Ltd/equivalent") =} d'abord (plus de protection). Ecris ta decision et ton calendrier.

4. **Verifie tes dependances.** Lance le verificateur de licences sur ton projet. Resous toute dependance GPL/AGPL avant de vendre un produit commercial.

---

## Lecon 5 : Canaux de Distribution Qui Fonctionnent en 2026

*"Le construire c'est 20% du travail. Le mettre devant les gens c'est les autres 80%."*

### La Realite de la Distribution

La plupart des produits developpeurs echouent non pas parce qu'ils sont mauvais, mais parce que personne ne sait qu'ils existent. La distribution — mettre ton produit devant des clients potentiels — est la competence ou la plupart des developpeurs sont les plus faibles. Et c'est la competence qui compte le plus.

Voici sept canaux de distribution classes par effort, calendrier et retour attendu. Tu n'as pas besoin des sept. Choisis 2-3 qui correspondent a tes forces et ton audience.

### Canal 1 : Hacker News

**Effort :** Eleve | **Calendrier :** Instantane (0-48 heures) | **Nature :** Tout-ou-rien

Hacker News (https://news.ycombinator.com) est le canal de distribution a evenement unique avec le plus grand effet de levier pour les produits developpeurs. Un post Show HN en premiere page peut envoyer 5 000-30 000 visiteurs en 24 heures. Mais c'est imprevisible — la plupart des posts n'obtiennent aucune traction.

**Ce qui fonctionne sur HN :**
- Produits techniques avec des details d'implementation interessants
- Outils axes sur la vie privee (l'audience HN se soucie profondement de la vie privee)
- Outils open-source avec un tier payant
- Solutions innovantes a des problemes connus
- Produits avec des demos en direct

**Ce qui ne fonctionne pas sur HN :**
- Lancements lourds en marketing ("Revolutionnaire IA-powered...")
- Produits qui sont des wrappers autour d'autres produits sans valeur originale
- Tout ce qui ressemble a une pub

**Le Manuel Show HN :**

```
AVANT DE PUBLIER :
1. Etudie les posts Show HN recents reussis dans ta categorie
   https://hn.algolia.com — filtre par "Show HN", trie par points
2. Prepare le titre de ton post : "Show HN: [Nom] – [ce que ca fait, <70 caracteres]"
   Bien : "Show HN: ScrubLog – Strip PII from Log Files in One Command"
   Mal : "Show HN: Introducing ScrubLog, the AI-Powered Log Anonymization Platform"
3. Aie une demo en direct prete (les lecteurs HN veulent essayer, pas lire)
4. Prepare des reponses aux questions probables (decisions techniques, justification des prix)

PUBLICATION :
5. Publie entre 7-9h du matin heure de l'Est US, mardi a jeudi
   (plus fort trafic, plus grande chance de traction)
6. Le corps de ton post devrait faire 4-6 paragraphes :
   - Ce que c'est (1 paragraphe)
   - Pourquoi tu l'as construit (1 paragraphe)
   - Details techniques (1-2 paragraphes)
   - Ce que tu cherches (feedback, questions specifiques)

APRES PUBLICATION :
7. Reste en ligne 4 heures apres la publication. Reponds a CHAQUE commentaire.
8. Sois humble et technique. HN recompense l'honnetete sur les limitations.
9. Si quelqu'un trouve un bug, corrige-le en direct et reponds "Corrige, merci."
10. Ne demande pas a des amis de voter. HN a une detection des cercles de votes.
```

**Resultats attendus (realistes) :**
- 70% des posts Show HN : <10 points, <500 visiteurs
- 20% des posts Show HN : 10-50 points, 500-3 000 visiteurs
- 10% des posts Show HN : 50+ points, 3 000-30 000 visiteurs

C'est une loterie avec des chances ponderees par l'effort. Un super produit avec un super post a peut-etre 30% de chance de traction significative. Pas garanti. Mais le potentiel est enorme.

### Canal 2 : Reddit

**Effort :** Moyen | **Calendrier :** 1-7 jours | **Nature :** Durable, repetable

Reddit est le canal de distribution le plus constant pour les produits developpeurs. Contrairement a HN (un seul tir), Reddit a des centaines de subreddits de niche ou ton produit est pertinent.

**Selection de subreddit :**

```
Subreddits developpeurs generaux :
- r/SideProject (140K+ membres) — fait pour ca
- r/webdev (2.4M membres) — enorme, competitif
- r/programming (6.3M membres) — tres competitif, axe actualites
- r/selfhosted (400K+ membres) — si ton produit est auto-hebergeable

Specifiques framework/langage :
- r/reactjs, r/nextjs, r/sveltejs, r/vuejs — pour les outils frontend
- r/rust, r/golang, r/python — pour les outils specifiques a un langage
- r/node — pour les outils et paquets Node.js

Specifiques domaine :
- r/devops — pour les outils d'infrastructure/deploiement
- r/machinelearning — pour les outils AI/ML
- r/datascience — pour les outils de donnees
- r/sysadmin — pour les outils d'admin/monitoring

La longue traine :
- Cherche des subreddits lies a ta niche specifique
- Les subreddits plus petits (10K-50K membres) ont souvent de meilleurs
  taux de conversion que les enormes
```

**Regles d'engagement Reddit :**

1. **Aie un vrai historique Reddit** avant de publier ton produit. Les comptes qui ne publient que de l'auto-promotion sont signales et shadowbannis.
2. **Suis les regles de chaque subreddit** sur l'auto-promotion. La plupart l'autorisent tant que tu es un membre contributeur.
3. **Interagis genuinement.** Reponds aux questions, apporte de la valeur, sois utile dans les commentaires d'autres posts. Puis partage ton produit.
4. **Publie a differentes heures** pour differents subreddits. Verifie https://later.com/reddit ou des outils similaires pour les heures de pointe.

**Resultats attendus (realistes) :**
- Post r/SideProject : 20-100 votes, 200-2 000 visiteurs
- Subreddit de niche (50K membres) : 10-50 votes, 100-1 000 visiteurs
- Premiere page de r/webdev : 100-500 votes, 2 000-10 000 visiteurs

### Canal 3 : Twitter/X

**Effort :** Moyen | **Calendrier :** 2-4 semaines pour prendre de l'elan | **Nature :** Se compose avec le temps

Twitter est un canal a construction lente. Ton premier tweet de lancement obtiendra 5 likes de tes amis. Mais si tu partages ton processus de construction de maniere constante, ton audience se compose.

**La Strategie Build-in-Public :**

```
Semaine 1 : Commence a partager ton processus de construction (avant le lancement)
- "Working on a [type de produit]. Here's the problem I'm solving: [capture]"
- "Day 3 of building [produit]. Got [fonctionnalite] working: [GIF/capture]"

Semaine 2 : Partage des insights techniques de la construction
- "TIL you need to [lecon technique] when building [type de produit]"
- "Architecture decision: chose [X] over [Y] because [raison]"

Semaine 3 : Lancement
- Fil de lancement (format de la Lecon 1)
- Partage des metriques specifiques : "Day 1: X visitors, Y signups"

Semaine 4+ : Continu
- Partage du feedback client (avec permission)
- Partage des jalons de revenus (les gens adorent les vrais chiffres)
- Partage des defis et comment tu les as resolus
```

**Avec qui interagir :**
- Suis et interagis avec des developpeurs dans ta niche
- Reponds aux tweets de comptes plus grands avec des commentaires reflechis (pas d'auto-promotion)
- Rejoins des Twitter Spaces sur ton domaine
- Cite des tweets de discussions pertinentes avec ta perspective

**Resultats attendus (realistes) :**
- 0-500 abonnes : Tweets de lancement obtiennent 5-20 likes, <100 visiteurs
- 500-2 000 abonnes : Tweets de lancement obtiennent 20-100 likes, 100-500 visiteurs
- 2 000-10 000 abonnes : Tweets de lancement obtiennent 100-500 likes, 500-5 000 visiteurs

Twitter est un investissement de 6 mois, pas une strategie du jour de lancement. Commence maintenant, meme avant que ton produit soit pret.

### Canal 4 : Product Hunt

**Effort :** Eleve | **Calendrier :** 1 jour d'activite intense | **Nature :** Boost unique

Product Hunt (https://producthunt.com) est une plateforme de lancement dediee. Un top-5 journalier peut envoyer 3 000-15 000 visiteurs. Mais ca necessite de la preparation.

**Checklist de Lancement Product Hunt :**

```
2 SEMAINES AVANT :
- [ ] Cree un profil maker Product Hunt
- [ ] Construis ton listing PH : slogan, description, images, video
- [ ] Prepare 4-5 captures d'ecran/GIFs de haute qualite
- [ ] Ecris un "premier commentaire" qui explique ta motivation
- [ ] Aligne 10-20 personnes pour soutenir le jour du lancement (pas de faux votes —
      de vraies personnes qui essaieront le produit et laisseront des commentaires genuins)
- [ ] Trouve un "hunter" (quelqu'un avec un grand following PH pour soumettre ton produit)
      ou soumets toi-meme

JOUR DE LANCEMENT (00:01 Pacific Time) :
- [ ] Sois en ligne des minuit PT. PH se reinitialise a minuit.
- [ ] Publie ton "premier commentaire" immediatement
- [ ] Partage le lien PH sur Twitter, LinkedIn, email, Discord
- [ ] Reponds a CHAQUE commentaire sur ton listing PH
- [ ] Publie des mises a jour tout au long de la journee ("Je viens de livrer un fix pour [X] !")
- [ ] Surveille toute la journee jusqu'a minuit PT

APRES :
- [ ] Remercie tous ceux qui ont soutenu
- [ ] Ecris un post "lecons apprises" (bon contenu pour Twitter/blog)
- [ ] Integre le badge PH sur ta landing page (preuve sociale)
```

> **Erreur Courante :** Lancer sur Product Hunt avant que ton produit soit pret. PH te donne un seul tir. Une fois que tu lances un produit, tu ne peux pas le relancer. Attends que ton produit soit poli, que ta landing page convertisse et que ton flux de paiement fonctionne. PH devrait etre ton "grand lancement" — pas ton lancement doux.

**Resultats attendus (realistes) :**
- Top 5 journalier : 3 000-15 000 visiteurs, 50-200 votes
- Top 10 journalier : 1 000-5 000 visiteurs, 20-50 votes
- En dessous du top 10 : <1 000 visiteurs. Impact durable minimal.

### Canal 5 : Dev.to / Hashnode / Articles de Blog Techniques

**Effort :** Faible-moyen | **Calendrier :** Resultats SEO en 1-3 mois | **Nature :** Longue traine, se compose indefiniment

Ecris des articles de blog techniques qui resolvent des problemes lies a ton produit, et mentionne ton produit comme solution.

**Strategie de contenu :**

```
Pour chaque produit, ecris 3-5 articles de blog :

1. "Comment [resoudre le probleme que ton produit resout] en 2026"
   - Enseigne l'approche manuelle, puis mentionne ton produit comme raccourci

2. "J'ai construit [produit] en 48 heures — voici ce que j'ai appris"
   - Contenu build-in-public. Details techniques + reflexion honnete.

3. "[Concurrent] vs [Ton Produit] : Comparaison Honnete"
   - Sois genuinement juste. Mentionne ou le concurrent gagne.
   - Ca capture le trafic de recherche de comparaison d'achats.

4. "[Concept technique lie a ton produit] explique"
   - Education pure. Mentionne ton produit une fois a la fin.

5. "Les outils que j'utilise pour [le domaine de ton produit] en 2026"
   - Format liste. Inclus ton produit aux cotes des autres.
```

**Ou publier :**
- **Dev.to** (https://dev.to) — Grande audience developpeur, bon SEO, gratuit
- **Hashnode** (https://hashnode.com) — Bon SEO, option domaine personnalise, gratuit
- **Ton propre blog** — Le meilleur pour le SEO long terme, tu possedes le contenu
- **Cross-publie partout.** Ecris une fois, publie sur les trois. Utilise des URLs canoniques pour eviter les penalites SEO.

**Resultats attendus par article :**
- Jour 1 : 100-1 000 vues (distribution de la plateforme)
- Mois 1-3 : 50-200 vues/mois (trafic de recherche en construction)
- Mois 6+ : 100-500 vues/mois (trafic de recherche compose)

Un seul article de blog bien ecrit peut generer 200+ visiteurs par mois pendant des annees. Cinq articles generent 1 000+/mois. Ca se compose.

### Canal 6 : Approche Directe

**Effort :** Eleve | **Calendrier :** Immediat | **Nature :** Taux de conversion le plus eleve

Les emails froids et DMs ont le taux de conversion le plus eleve de tous les canaux — mais aussi le plus grand effort par lead. Utilise ca pour les produits plus chers ($99+) ou les ventes B2B.

**Modele d'email pour contacter des clients potentiels :**

```
Objet : Petite question sur [leur point de douleur specifique]

Salut [nom],

J'ai vu ton [tweet/post/commentaire] sur [probleme specifique qu'ils ont mentionne].

J'ai construit [nom du produit] specifiquement pour ca — ca [description
en une phrase de ce que ca fait].

Tu serais ouvert a l'essayer ? Je serais ravi de te donner un acces gratuit
en echange de feedback.

[Ton nom]
[Lien vers le produit]
```

**Regles pour l'approche froide :**
- Ne contacte que des personnes qui ont publiquement exprime le probleme que ton produit resout
- Reference leur post/commentaire specifique (prouve que tu n'envoies pas des emails en masse)
- Offre de la valeur (acces gratuit, reduction) plutot que de demander de l'argent immediatement
- Garde ca sous 5 phrases
- Envoie depuis une vraie adresse email (toi@tondomaine.com, pas gmail)
- Relance une fois apres 3-4 jours. Pas de reponse ? Arrete.

**Resultats attendus :**
- Taux de reponse : 10-20% (email froid a des destinataires pertinents)
- Conversion de reponse a essai : 30-50%
- Conversion d'essai a payant : 20-40%
- Conversion effective : 1-4% des personnes contactees deviennent clients

Pour un produit a $99, contacter 100 personnes = 1-4 ventes = $99-396. Pas scalable, mais excellent pour obtenir des premiers clients et du feedback.

### Canal 7 : SEO

**Effort :** Faible continu | **Calendrier :** 3-6 mois pour les resultats | **Nature :** Se compose indefiniment

Le SEO est le meilleur canal de distribution long terme. Il est lent au demarrage mais une fois qu'il fonctionne, il envoie du trafic gratuit indefiniment.

**Strategie SEO axee developpeurs :**

```
1. Cible les mots-cles long-tail (plus faciles a classer) :
   Au lieu de : "dashboard components"
   Cible : "tailwind dashboard components react typescript"

2. Cree une page par mot-cle :
   Chaque article de blog ou page de docs cible une requete de recherche specifique

3. Implementation technique :
   - Utilise la generation de site statique (Astro, Next.js SSG) pour des chargements rapides
   - Ajoute des meta descriptions a chaque page
   - Utilise du HTML semantique (hierarchie h1, h2, h3)
   - Ajoute du texte alt a chaque image
   - Soumets le sitemap a Google Search Console

4. Contenu qui se classe pour les outils developpeurs :
   - Pages de documentation (etonnamment bonnes pour le SEO)
   - Pages de comparaison ("X vs Y")
   - Pages tutoriel ("Comment faire X avec Y")
   - Pages changelog (contenu frais signale a Google)
```

```bash
# Soumets ton sitemap a Google Search Console
# 1. Va a https://search.google.com/search-console
# 2. Ajoute ta propriete (domaine ou prefixe d'URL)
# 3. Verifie la propriete (enregistrement DNS TXT ou fichier HTML)
# 4. Soumets l'URL de ton sitemap : tondomaine.com/sitemap.xml

# Si tu utilises Astro :
pnpm add @astrojs/sitemap
# Le sitemap est auto-genere a /sitemap.xml

# Si tu utilises Next.js, ajoute a next-sitemap.config.js :
# pnpm add next-sitemap
```

**Resultats attendus :**
- Mois 1-3 : Trafic organique minimal (<100/mois)
- Mois 3-6 : Trafic croissant (100-500/mois)
- Mois 6-12 : Trafic significatif (500-5 000/mois)
- Mois 12+ : Trafic compose qui croit sans effort

{@ temporal market_timing @}

### Cadre de Selection de Canaux

Tu ne peux pas tous les faire bien. Choisis 2-3 base sur cette matrice :

| Si tu es... | Priorise | Saute |
|---|---|---|
| En train de lancer ce week-end | Reddit + HN | SEO, Twitter (trop lent) |
| En train de construire une audience d'abord | Twitter + Articles de blog | Approche directe, PH |
| En train de vendre un produit a $99+ | Approche directe + HN | Dev.to (l'audience attend du gratuit) |
| En train de jouer le long terme | SEO + Articles de blog + Twitter | PH (un seul tir, utilise plus tard) |
| Non anglophone | Dev.to + Reddit (global) | HN (centre US) |

### A Ton Tour

1. **Choisis tes 2-3 canaux.** Base sur la matrice ci-dessus et ton type de produit, choisis les canaux sur lesquels tu te concentreras. Ecris-les avec ton calendrier prevu pour chacun.

2. **Ecris ton post Reddit.** En utilisant le modele de la Lecon 1, ecris ton brouillon de post r/SideProject maintenant. Sauvegarde-le. Tu le publieras le jour du lancement.

3. **Ecris ton premier article de blog.** Redige un article "Comment [resoudre le probleme que ton produit resout]". Ca va sur Dev.to ou ton blog dans la premiere semaine de lancement. Vise 1 500-2 000 mots.

4. **Configure Google Search Console.** Ca prend 5 minutes et te donne des donnees SEO des le premier jour. Fais-le avant de lancer pour avoir des donnees de reference.

---

## Lecon 6 : Ta Checklist de Lancement

*"L'espoir n'est pas une strategie de lancement. Les checklists, si."*

### La Checklist Pre-Lancement

Parcours chaque element. Ne lance pas tant que chaque element "Requis" n'est pas coche. Les elements "Recommandes" peuvent etre faits en Semaine 1 si necessaire.

**Produit (Requis) :**

```
- [ ] La fonctionnalite de base fonctionne comme decrit sur la landing page
- [ ] Pas de bugs critiques dans le flux achat -> livraison
- [ ] Fonctionne dans Chrome, Firefox et Safari (pour les produits web)
- [ ] Landing page responsive mobile (50%+ du trafic est mobile)
- [ ] Les messages d'erreur sont utiles, pas des stack traces
- [ ] Etats de chargement pour toute operation asynchrone
```

**Landing Page (Requis) :**

```
- [ ] Titre clair : ce que ca fait en 8 mots ou moins
- [ ] Enonce du probleme : 3 points de douleur dans le langage du client
- [ ] Section solution : captures d'ecran ou demos du produit
- [ ] Tarifs : visibles, clairs, avec bouton d'achat
- [ ] Appel a l'action : un bouton principal, visible sans scroller
- [ ] Politique de confidentialite liee dans le pied de page
- [ ] Conditions d'utilisation liees dans le pied de page
```

**Paiements (Requis) :**

```
- [ ] Flux de checkout teste de bout en bout en mode test
- [ ] Flux de checkout teste de bout en bout en mode reel (achat test de $1)
- [ ] Le webhook recoit la confirmation de paiement
- [ ] Le client recoit l'acces au produit apres le paiement
- [ ] Processus de remboursement documente (tu RECEVRAS des demandes de remboursement)
- [ ] Recu/facture envoye automatiquement
```

**Infrastructure (Requis) :**

```
- [ ] Domaine personnalise pointant vers le site en ligne
- [ ] HTTPS fonctionnel (cadenas vert)
- [ ] Monitoring d'uptime actif
- [ ] Script analytics installe et recevant des donnees
- [ ] Email de contact fonctionnel (toi@tondomaine.com)
```

**Distribution (Requis) :**

```
- [ ] Post Reddit redige et pret
- [ ] Post Show HN redige et pret (si applicable)
- [ ] Fil de lancement Twitter redige
- [ ] 2-3 communautes identifiees pour le partage
```

**Recommande (Semaine 1) :**

```
- [ ] Balises meta OpenGraph pour les apercus de partage social
- [ ] Page 404 personnalisee
- [ ] Page ou section FAQ
- [ ] Sequence d'emails d'onboarding client (bienvenue + premiers pas)
- [ ] Page changelog (meme si vide — montre l'engagement envers les mises a jour)
- [ ] Article de blog : "J'ai construit [produit] en 48 heures"
- [ ] Google Search Console verifie et sitemap soumis
```

### Elements d'Action Post-Lancement

**Jour 1 (Jour de Lancement) :**

```
Matin :
- [ ] Publie sur Reddit (r/SideProject + 1 subreddit de niche)
- [ ] Publie Show HN (si applicable)
- [ ] Publie le fil de lancement Twitter

Toute la journee :
- [ ] Reponds a CHAQUE commentaire sur Reddit, HN et Twitter
- [ ] Surveille les logs d'erreur et les analytics en temps reel
- [ ] Corrige tout bug decouvert par les utilisateurs immediatement
- [ ] Envoie un email de remerciement personnel a chaque client

Soir :
- [ ] Verifie les metriques : visiteurs, taux de conversion, revenus
- [ ] Capture d'ecran de ton dashboard analytics (tu le voudras plus tard)
- [ ] Ecris les 3 pieces de feedback les plus courantes
```

**Semaine 1 :**

```
- [ ] Reponds a tout le feedback et les demandes de support dans les 24 heures
- [ ] Corrige les 3 principaux bugs/problemes identifies pendant le lancement
- [ ] Ecris et publie ton premier article de blog
- [ ] Envoie un email de suivi a tous les clients demandant du feedback
- [ ] Revise les analytics : quelles pages ont les taux de rebond les plus eleves ?
- [ ] Configure une methode simple de collecte de feedback (email, Typeform ou Canny)

Metriques hebdomadaires a enregistrer :
| Metrique               | Cible     | Reel   |
|------------------------|-----------|--------|
| Visiteurs uniques      | 500+      |        |
| Taux de clic checkout  | 2-5%      |        |
| Conversion d'achat     | 1-3%      |        |
| Revenus                | $50+      |        |
| Demandes de support    | <10       |        |
| Demandes de remboursement | <2     |        |
```

**Mois 1 :**

```
- [ ] Livre 4 ameliorations hebdomadaires basees sur le feedback client
- [ ] Publie 2+ articles de blog (construction SEO)
- [ ] Collecte 3+ temoignages de clients
- [ ] Ajoute les temoignages a la landing page
- [ ] Evalue les tarifs : trop hauts ? trop bas ? (revise les donnees de conversion)
- [ ] Planifie ton "grand lancement" sur Product Hunt (si applicable)
- [ ] Commence a construire une liste email pour les futurs lancements de produits
- [ ] Revise et ajuste ta strategie de canaux de distribution

Revue financiere mensuelle :
| Categorie                  | Montant   |
|----------------------------|-----------|
| Revenu brut                | $         |
| Frais du processeur de paiement | $   |
| Couts d'hebergement/infra  | $         |
| Couts API                  | $         |
| Benefice net               | $         |
| Heures investies           |           |
| Taux horaire effectif      | $         |
```

### Le Dashboard de Metriques

Configure un dashboard de metriques simple que tu verifies quotidiennement. Ca n'a pas besoin d'etre sophistique — un tableur fonctionne.

```
=== METRIQUES QUOTIDIENNES (verifie chaque matin) ===

Date : ___
Visiteurs hier : ___
Nouveaux clients hier : ___
Revenus hier : $___
Demandes de support : ___
Uptime : ___%

=== METRIQUES HEBDOMADAIRES (verifie chaque lundi) ===

Semaine du : ___
Visiteurs totaux : ___
Clients totaux : ___
Revenus totaux : $___
Taux de conversion : ___% (clients / visiteurs)
Page la plus visitee : ___
Principale source de trafic : ___
Principal theme de feedback : ___

=== METRIQUES MENSUELLES (verifie le 1er du mois) ===

Mois : ___
Revenus totaux : $___
Depenses totales : $___
Benefice net : $___
Clients totaux : ___
Remboursements : ___
Taux de depart (abonnements) : ___%
MRR (Revenu Mensuel Recurrent) : $___
Taux de croissance vs. mois precedent : ___%
```

**Configuration analytics respectueuse de la vie privee :**

```javascript
// Si tu utilises Plausible, tu obtiens la plupart de ca dans leur dashboard.
// Pour le tracking d'evenements personnalises :

// Tracker les clics checkout
document.querySelector('#buy-button').addEventListener('click', () => {
  plausible('Checkout Click', {
    props: { tier: 'pro', price: '59' }
  });
});

// Tracker les achats reussis (appelle depuis ton gestionnaire de succes webhook)
plausible('Purchase', {
  props: { tier: 'pro', revenue: '59' }
});
```

### Quand Doubler la Mise, Pivoter ou Arreter

Apres 30 jours de donnees, tu as assez de signal pour prendre une decision :

**Doubler la Mise (continue, investis plus) :**

```
Signaux :
- Les revenus croissent semaine apres semaine (meme lentement)
- Les clients fournissent des demandes de fonctionnalites specifiques (ils veulent PLUS)
- Le taux de conversion est stable ou en amelioration
- Tu obtiens du trafic organique (des gens te trouvent sans tes posts)
- Au moins un client a dit "ca m'a economise [du temps/de l'argent]"

Actions :
- Augmente les efforts de distribution (ajoute un canal)
- Livre la fonctionnalite la plus demandee
- Augmente les prix legerement
- Commence a construire une liste email pour les futurs lancements
```

**Pivoter (change l'angle, garde le noyau) :**

```
Signaux :
- Des visiteurs mais pas de ventes (les gens sont interesses mais n'achetent pas)
- Des ventes d'une audience inattendue (des personnes differentes de celles que tu ciblais)
- Les clients utilisent le produit differemment de ce que tu attendais
- Le feedback pointe constamment vers un probleme different de celui que tu resous

Actions :
- Reecris la landing page pour l'audience/cas d'utilisation reel
- Ajuste les tarifs base sur la volonte de payer de l'audience reelle
- Repriorise les fonctionnalites vers ce que les gens utilisent reellement
- Garde le code, change le positionnement
```

**Arreter (stoppe, apprends, construis autre chose) :**

```
Signaux :
- Pas de visiteurs malgre les efforts de distribution (probleme de demande)
- Des visiteurs mais zero clics checkout (probleme de positionnement/prix
  qui persiste apres les ajustements)
- Revenus stagnants depuis 4+ semaines sans tendance de croissance
- Tu redoutes de travailler dessus (la motivation compte pour les produits solo)
- Le marche a change (un concurrent a lance, la technologie a change)

Actions :
- Ecris un post-mortem : ce qui a fonctionne, ce qui n'a pas, ce que tu as appris
- Garde le code — des morceaux pourraient etre utiles dans ton prochain produit
- Prends une semaine sans construire
- Demarre le processus de validation pour une nouvelle idee
- Ce n'est pas un echec. Ce sont des donnees. La plupart des produits ne fonctionnent pas.
  Les developpeurs qui gagnent de l'argent sont ceux qui livrent 5 produits,
  pas ceux qui passent un an sur un.
```

### Le Modele de Document de Lancement

C'est ton livrable pour le Module E. Cree ce document et remplis-le pendant que tu executes ton lancement.

```markdown
# Document de Lancement : [Nom du Produit]

## Pre-Lancement

### Resume de Validation
- **Volume de recherche :** [chiffres de Google Trends/Ahrefs]
- **Preuves de fils :** [liens vers 5+ fils montrant la demande]
- **Audit des concurrents :** [3+ concurrents avec forces/faiblesses]
- **Preuve "10 personnes paieraient" :** [comment tu as valide]

### Produit
- **URL :** [URL du produit en ligne]
- **Domaine :** [domaine achete]
- **Hebergement :** [plateforme]
- **Fonctionnalites de base (v0.1) :**
  1. [Fonctionnalite 1]
  2. [Fonctionnalite 2]
  3. [Fonctionnalite 3]

### Tarifs
- **Prix :** $[montant]
- **Structure de niveaux :** [Basique/Pro/Equipe ou niveau unique]
- **Plateforme de paiement :** [Lemon Squeezy/Stripe]
- **URL de checkout :** [lien]

### Juridique
- **Politique de confidentialite :** [URL]
- **Conditions d'utilisation :** [URL]
- **Entite commerciale :** [type ou "entrepreneur individuel"]

## Lancement

### Canaux de Distribution
| Canal   | URL du Post | Date de Publication | Resultats |
|---------|-------------|---------------------|-----------|
| Reddit  | [lien]      | [date]              | [visiteurs, votes] |
| HN      | [lien]      | [date]              | [visiteurs, points] |
| Twitter | [lien]      | [date]              | [impressions, clics] |

### Metriques Jour 1
- Visiteurs : ___
- Clics checkout : ___
- Achats : ___
- Revenus : $___

### Metriques Semaine 1
- Visiteurs totaux : ___
- Achats totaux : ___
- Revenus totaux : $___
- Taux de conversion : ___%
- Principal feedback : ___

### Metriques Mois 1
- Revenus totaux : $___
- Depenses totales : $___
- Benefice net : $___
- Clients totaux : ___
- Decision : [ ] Doubler la mise [ ] Pivoter [ ] Arreter

## Feuille de Route Post-Lancement
- Semaine 2 : [amelioration prevue]
- Semaine 3 : [amelioration prevue]
- Semaine 4 : [amelioration prevue]
- Mois 2 : [fonctionnalite/expansion prevue]

## Lecons Apprises
- Ce qui a fonctionne : ___
- Ce qui n'a pas fonctionne : ___
- Ce que je ferais differemment : ___
```

### Integration 4DA

> **Integration 4DA :** Les signaux actionnables de 4DA classifient le contenu par urgence. Un signal "critique" sur une vulnerabilite dans un paquet populaire signifie : construis le correctif ou l'outil de migration MAINTENANT, avant quiconque. Un signal de "tendance montante" sur un nouveau framework signifie : construis le kit de demarrage ce week-end pendant que la concurrence est quasi nulle. Le sprint de 48 heures de la Lecon 1 fonctionne mieux quand ton idee vient d'un signal sensible au temps. Connecte ton flux d'intelligence 4DA a ton calendrier de sprints — quand une opportunite a haute urgence apparait, bloque le prochain week-end et execute. La difference entre les developpeurs qui capturent les opportunites et ceux qui les ratent n'est pas le talent. C'est la vitesse. 4DA te donne le radar. Ce module te donne la sequence de lancement. Ensemble, ils transforment des signaux en revenus.

### A Ton Tour

1. **Complete la checklist pre-lancement.** Parcours chaque element. Marque chacun comme fait ou programme quand tu le feras. Ne saute pas les elements "Requis".

2. **Cree ton Document de Lancement.** Copie le modele ci-dessus dans ton outil de documents prefere. Remplis tout ce que tu sais maintenant. Laisse des blancs pour les metriques que tu rempliras pendant et apres le lancement.

3. **Fixe ta date de lancement.** Ouvre ton calendrier. Choisis un samedi specifique dans les 2 prochaines semaines. Ecris-le. Dis-le a quelqu'un — un ami, un partenaire, un abonne Twitter. La responsabilite rend ca reel.

4. **Fixe tes criteres d'arret.** Avant de lancer, decide : "Si j'ai moins de [X] ventes apres 30 jours malgre [Y] effort de distribution, je [pivoterai/arreterai]." Ecris ca dans ton Document de Lancement. Avoir des criteres pre-engages t'empeche d'investir des mois dans un produit mort a cause du biais du cout irrecuperable.
{? if progress.completed("S") ?}
   Reporte-toi a ton Document de Stack Souverain du Module S — tes contraintes de budget et couts operationnels definissent ce que "rentable" signifie pour ta situation specifique.
{? endif ?}

5. **Lance-le.** Tu as le manuel. Tu as les outils. Tu as le savoir. La seule chose qui reste est l'acte. Internet attend.

---

## Module E : Termine

### Ce Que Tu As Construit en Deux Semaines

{? if dna.identity_summary ?}
> **Ton identite developpeur :** {= dna.identity_summary | fallback("Pas encore profilee") =}. Tout ce que tu as construit dans ce module exploite cette identite — ta vitesse de livraison est une fonction de ton expertise existante.
{? endif ?}

Regarde ce que tu as maintenant que tu n'avais pas quand tu as commence ce module :

1. **Un cadre d'execution de 48 heures** que tu peux repeter pour chaque produit que tu construis — d'une idee validee a un produit en ligne en un week-end.
2. **Un etat d'esprit de livraison** qui priorise l'existence sur la perfection, les donnees sur les devinettes, et l'iteration sur la planification.
3. **Une strategie de tarification** fondee sur une vraie psychologie et de vrais chiffres, pas de l'espoir et de la sous-tarification.
4. **Une base juridique** qui te protege sans te paralyser — politique de confidentialite, conditions, plan d'entite.
5. **Un manuel de distribution** avec des modeles specifiques, du timing et des resultats attendus pour sept canaux.
6. **Une checklist de lancement et un systeme de suivi** qui transforment le chaos en processus — repetable, mesurable, ameliorable.
7. **Un produit en ligne, acceptant les paiements, avec de vrais humains qui le visitent.**

Ce dernier est celui qui compte. Tout le reste est de la preparation. Le produit est la preuve.

### Ce Qui Vient Ensuite : Module E2 — L'Avantage en Evolution

Le Module E1 t'a amene au lancement. Le Module E2 te maintient en tete.

Voici ce que couvre le Module E2 :

- **Systemes de detection de tendances** — comment reperer les opportunites 2-4 semaines avant qu'elles deviennent evidentes
- **Surveillance concurrentielle** — suivre ce que les autres dans ton espace construisent et tarifent
- **Surfer sur les vagues technologiques** — quand adopter de la nouvelle tech dans tes produits et quand attendre
- **Developpement client** — transformer tes 10 premiers clients en ton conseil consultatif produit
- **La decision du deuxieme produit** — quand construire le produit #2 vs. ameliorer le produit #1

Les developpeurs qui generent des revenus constants ne sont pas ceux qui lancent une fois. Ce sont ceux qui lancent, iterent et restent en avance sur le marche. Le Module E2 te donne le systeme pour rester en avance.

### La Feuille de Route Complete STREETS

| Module | Titre | Focus | Duree |
|--------|-------|-------|-------|
| **S** | Configuration Souveraine | Infrastructure, juridique, budget | Semaines 1-2 |
| **T** | Fosses Techniques | Avantages defensibles, actifs proprietaires | Semaines 3-4 |
| **R** | Moteurs de Revenus | Manuels de monetisation specifiques avec code | Semaines 5-8 |
| **E** | Manuel d'Execution | Sequences de lancement, tarifs, premiers clients | Semaines 9-10 (termine) |
| **E** | Avantage en Evolution | Rester en tete, detection de tendances, adaptation | Semaines 11-12 |
| **T** | Automatisation Tactique | Automatiser les operations pour des revenus passifs | Semaines 13-14 |
| **S** | Empiler les Sources | Sources de revenus multiples, strategie de portefeuille | Semaines 15-16 |

Tu as depasse le point median. Tu as un produit en ligne. Ca te place devant 95% des developpeurs qui veulent construire des revenus independants mais n'arrivent jamais aussi loin.

> **Progression STREETS :** {= progress.completed_count | fallback("0") =} sur {= progress.total_count | fallback("7") =} modules termines. {? if progress.completed_modules ?}Termines : {= progress.completed_modules | fallback("Aucun pour l'instant") =}.{? endif ?}

Maintenant fais-le grandir.

---

**Ton produit est en ligne. Ton checkout fonctionne. Des humains peuvent te payer de l'argent.**

**Tout apres ca est de l'optimisation. Et l'optimisation est la partie amusante.**

*Ton equipe. Tes regles. Tes revenus.*
