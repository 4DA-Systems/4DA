# Modulo E: Manual de Execucao

**Curso STREETS de Renda para Desenvolvedores — Modulo Pago**
*Semanas 9-10 | 6 Licoes | Entregavel: Seu Primeiro Produto, No Ar e Aceitando Pagamentos*

> "Da ideia ao deploy em 48 horas. Sem overthinking."

---

Voce tem a infraestrutura (Modulo S). Voce tem o fosso competitivo (Modulo T). Voce tem os designs do motor de receita (Modulo R). Agora e hora de lancar.

Este modulo e o que a maioria dos desenvolvedores nunca alcanca — nao porque e dificil, mas porque eles ainda estao polindo o codigo, refatorando a arquitetura, ajustando a paleta de cores. Eles estao fazendo tudo exceto a unica coisa que importa: colocar um produto na frente de um ser humano que pode pagar por ele.

Lancar e uma habilidade. Como qualquer habilidade, fica mais facil com pratica e pior com atraso. Quanto mais voce espera, mais dificil se torna. Quanto mais voce lanca, menos assustador parece. Seu primeiro lancamento vai ser bagunçado. Esse e o ponto.

Ao final dessas duas semanas, voce tera:

- Uma ideia de produto validada testada contra sinais reais de demanda
- Um produto no ar, deployado, acessivel por um dominio real
- Processamento de pagamentos aceitando dinheiro real
- Pelo menos um lancamento publico em uma plataforma onde seu publico-alvo se reune
- Um sistema de metricas pos-lancamento para guiar seus proximos passos

Sem hipoteticos. Sem "em teoria." Um produto real, no ar na internet, capaz de gerar receita.

{? if progress.completed("R") ?}
Voce completou o Modulo R — voce ja tem designs de motor de receita prontos para executar. Este modulo transforma um desses designs em um produto no ar.
{? else ?}
Se voce ainda nao completou o Modulo R, voce ainda pode usar este modulo — mas ter um design de motor de receita pronto vai tornar o sprint de 48 horas significativamente mais suave.
{? endif ?}

{@ mirror execution_readiness @}

Vamos construir.

---

## Licao 1: O Sprint de 48 Horas

*"Sabado de manha ate domingo a noite. Um produto. Zero desculpas."*

### Por Que 48 Horas

A Lei de Parkinson diz que o trabalho se expande para preencher o tempo disponivel. De a si mesmo 6 meses para construir um produto e voce vai gastar 5 meses deliberando e 1 mes em um frenesi estressado. De a si mesmo 48 horas e voce vai tomar decisoes, cortar escopo impiedosamente e lancar algo real.

A restricao de 48 horas nao e sobre construir algo perfeito. E sobre construir algo que exista. Existencia vence perfeicao sempre, porque um produto no ar gera dados — quem visita, quem clica, quem paga, quem reclama — e dados dizem o que construir em seguida.

Todo produto de desenvolvedor bem-sucedido que estudei seguiu esse padrao: lance rapido, aprenda rapido, itere rapido. Os que falharam? Todos tem READMEs lindos e zero usuarios.

Aqui esta seu playbook minuto a minuto.

### Dia 1 — Sabado

#### Bloco da Manha (4 horas): Validar Demanda

Antes de escrever uma unica linha de codigo, voce precisa de evidencias de que alguem alem de voce quer essa coisa. Nao certeza — evidencia. A diferenca importa. Certeza e impossivel. Evidencia e alcancavel em 4 horas.

**Passo 1: Verificacao de Volume de Busca (45 minutos)**

Va a essas fontes e pesquise pela sua ideia de produto e termos relacionados:

- **Google Trends** (https://trends.google.com) — Gratis. Mostra interesse de busca relativo ao longo do tempo. Voce quer ver uma linha estavel ou ascendente, nao uma em declinio.
- **Ahrefs Free Webmaster Tools** (https://ahrefs.com/webmaster-tools) — Gratis com verificacao de site. Mostra volumes de palavras-chave.
- **Ubersuggest** (https://neilpatel.com/ubersuggest/) — Tier gratis da 3 buscas/dia. Mostra volume de busca, dificuldade e termos relacionados.
- **AlsoAsked** (https://alsoasked.com) — Tier gratis. Mostra dados de "As Pessoas Tambem Perguntam" do Google. Revela quais perguntas as pessoas estao realmente fazendo.

O que voce esta procurando:

```
BONS sinais:
- 500+ buscas mensais para sua palavra-chave principal
- Tendencia ascendente nos ultimos 12 meses
- Multiplas perguntas "As Pessoas Tambem Perguntam" sem boas respostas
- Palavras-chave long-tail relacionadas com baixa concorrencia

MAUS sinais:
- Interesse de busca em declinio
- Zero volume de busca (ninguem esta procurando isso)
- Dominado por empresas enormes na pagina 1
- Sem variacao nos termos de busca (muito restrito)
```

Exemplo real: Suponha que sua ideia de motor de receita do Modulo R e uma "biblioteca de componentes Tailwind CSS para dashboards SaaS."

```
Busca: "tailwind dashboard components" — 2.900/mes, tendencia ascendente
Busca: "tailwind admin template" — 6.600/mes, estavel
Busca: "react dashboard template tailwind" — 1.300/mes, ascendente
Relacionados: "shadcn dashboard", "tailwind analytics components"

Veredito: Demanda forte. Multiplos angulos de palavras-chave. Prossiga.
```

Outro exemplo: Suponha que sua ideia e um "anonimizador de arquivos de log baseado em Rust."

```
Busca: "log file anonymizer" — 90/mes, estavel
Busca: "anonymize log files" — 140/mes, estavel
Busca: "PII removal from logs" — 320/mes, ascendente
Relacionados: "GDPR log compliance", "scrub PII from logs"

Veredito: Nicho mas crescente. O angulo "PII removal" tem mais volume
do que o angulo "anonymizer". Reposicione seu produto.
```

**Passo 2: Mineracao de Threads da Comunidade (60 minutos)**

Va aonde os desenvolvedores pedem coisas e pesquise pelo seu espaco de problema:

- **Reddit:** Pesquise em r/webdev, r/reactjs, r/selfhosted, r/SideProject, r/programming, e subreddits de nicho relevantes ao seu dominio
- **Hacker News:** Use https://hn.algolia.com para pesquisar discussoes passadas
- **GitHub Issues:** Pesquise por issues em repos populares relacionados ao seu espaco
- **Stack Overflow:** Pesquise por perguntas com muitos upvotes mas respostas aceitas insatisfatorias
- **Servidores Discord:** Verifique servidores de comunidades de desenvolvedores relevantes

O que voce esta documentando:

```markdown
## Resultados da Mineracao de Threads

### Thread 1
- **Fonte:** Reddit r/reactjs
- **URL:** [link]
- **Titulo:** "Existe algum bom kit de dashboard Tailwind que nao custe $200?"
- **Upvotes:** 147
- **Comentarios:** 83
- **Citacoes-chave:**
  - "Tudo no mercado e gratis e feio, ou $200+ e exagerado"
  - "Eu so preciso de 10-15 componentes bem desenhados, nao 500"
  - "Pagaria $49 por algo que realmente fique bom direto da caixa"
- **Conclusao:** Sensibilidade a preco em $200+, disposicao para pagar em $29-49

### Thread 2
- ...
```

Encontre pelo menos 5 threads. Se voce nao consegue encontrar 5 threads onde pessoas estao pedindo algo no espaco do seu produto, isso e um sinal de alerta serio. Ou a demanda nao existe, ou voce esta pesquisando com os termos errados. Tente palavras-chave diferentes antes de desistir da ideia.

**Passo 3: Auditoria de Concorrentes (45 minutos)**

Pesquise o que ja existe. Isso nao e desanimador — e validador. Concorrentes significam que existe um mercado. Sem concorrentes geralmente significa que nao ha mercado, nao que voce encontrou um oceano azul.

Para cada concorrente, documente:

```markdown
## Auditoria de Concorrentes

### Concorrente 1: [Nome]
- **URL:** [link]
- **Preco:** $XX
- **O que eles fazem bem:** [coisas especificas]
- **O que e ruim nele:** [reclamacoes especificas de reviews/threads]
- **Reviews deles:** [verifique G2, reviews no ProductHunt, mencoes no Reddit]
- **Seu angulo:** [como voce faria diferente]

### Concorrente 2: [Nome]
- ...
```

O ouro esta em "o que e ruim nele." Cada reclamacao sobre um concorrente e um pedido de feature para o seu produto. Pessoas literalmente dizendo o que construir e quanto cobrar.

**Passo 4: O Teste "10 Pessoas Pagariam" (30 minutos)**

Este e o portao final de validacao. Voce precisa encontrar evidencias de que pelo menos 10 pessoas pagariam dinheiro por isso. Nao "expressaram interesse." Nao "disseram que era legal." Pagariam.

Fontes de evidencia:
- Threads no Reddit onde pessoas dizem "eu pagaria por X" (sinal mais forte)
- Produtos concorrentes com clientes pagantes (prova que o mercado paga)
- Produtos no Gumroad/Lemon Squeezy no seu espaco com contagens de vendas visiveis
- Repos no GitHub com 1.000+ stars que resolvem um problema relacionado (pessoas valorizam isso o suficiente para dar star)
- Sua propria audiencia, se voce tem uma (tweete, mande DM para 10 pessoas, pergunte diretamente)

Se voce passou neste teste: prossiga. Construa.

Se voce nao passou neste teste: pivote seu angulo, nao sua ideia inteira. A demanda pode existir em um espaco adjacente. Tente posicionamento diferente antes de abandonar.

> **Papo Reto:** A maioria dos desenvolvedores pula a validacao inteiramente porque querem codar. Eles vao gastar 200 horas construindo algo que ninguem pediu, e depois se perguntam por que ninguem compra. Essas 4 horas de pesquisa vao te poupar 196 horas de esforco desperdicado. Nao pule isso. O codigo e a parte facil.

#### Bloco da Tarde (4 horas): Construir o MVP

Voce validou a demanda. Voce tem pesquisa de concorrentes. Voce sabe o que as pessoas querem e o que as solucoes existentes nao oferecem. Agora construa a versao minima que resolve o problema central.

{? if profile.gpu.exists ?}
Com uma GPU no seu setup ({= profile.gpu.model | fallback("sua GPU") =}), considere ideias de produto que aproveitem inferencia local de IA — ferramentas de processamento de imagem, utilitarios de analise de codigo, pipelines de geracao de conteudo. Features com GPU sao um diferencial genuino que a maioria dos desenvolvedores indie nao consegue oferecer.
{? endif ?}

**A Regra das 3 Features**

Seu v0.1 tem exatamente 3 features. Nao 4. Nao 7. Tres.

Como escolhe-las:
1. Qual e a UNICA coisa que seu produto faz? (Feature 1 — o nucleo)
2. O que o torna usavel? (Feature 2 — geralmente autenticacao, ou salvar/exportar, ou configuracao)
3. O que o torna digno de pagamento em relacao as alternativas? (Feature 3 — seu diferencial)

Todo o resto vai para uma lista "v0.2" que voce nao toca neste fim de semana.

Exemplo real — uma biblioteca de componentes de dashboard Tailwind:
1. **Nucleo:** 12 componentes de dashboard prontos para producao (graficos, tabelas, cards de estatisticas, navegacao)
2. **Usavel:** Trechos de codigo copy-paste com preview ao vivo
3. **Diferencial:** Dark mode integrado, componentes projetados para funcionar juntos (nao uma colecao aleatoria)

Exemplo real — uma ferramenta CLI de scrubbing de PII em logs:
1. **Nucleo:** Detectar e redigir PII de arquivos de log (emails, IPs, nomes, SSNs)
2. **Usavel:** Funciona como um pipe CLI (`cat logs.txt | pii-scrub > clean.txt`)
3. **Diferencial:** Arquivo de regras configuravel, lida com 15+ formatos de log automaticamente

{@ insight stack_fit @}

**Montar o Scaffold do Projeto**

Use LLMs para acelerar, nao substituir, seu trabalho. Aqui esta o fluxo pratico:

{? if stack.contains("react") ?}
Como seu stack principal inclui React, o scaffold de web app abaixo e seu caminho mais rapido. Voce ja conhece as ferramentas — foque suas 48 horas na logica do produto, nao em aprender um novo framework.
{? elif stack.contains("rust") ?}
Como seu stack principal inclui Rust, o scaffold de ferramenta CLI abaixo e seu caminho mais rapido. Ferramentas CLI em Rust tem excelente distribuicao (binario unico, multiplataforma) e audiencias de desenvolvedores respeitam a historia de performance.
{? elif stack.contains("python") ?}
Como seu stack principal inclui Python, considere uma ferramenta CLI ou servico de API. Python lanca rapido com FastAPI ou Typer, e o ecossistema PyPI te da distribuicao instantanea para milhoes de desenvolvedores.
{? endif ?}

```bash
# Scaffold de web app (ferramenta SaaS, biblioteca de componentes com site de docs, etc.)
pnpm create vite@latest my-product -- --template react-ts
cd my-product
pnpm install

# Adicionar Tailwind CSS (mais comum para produtos de desenvolvedor)
pnpm install -D tailwindcss @tailwindcss/vite

# Adicionar roteamento se voce precisa de multiplas paginas
pnpm install react-router-dom

# Estrutura do projeto — mantenha plana para uma build de 48 horas
mkdir -p src/components src/pages src/lib
```

```bash
# Scaffold de ferramenta CLI (para utilitarios de desenvolvedor)
cargo init my-tool
cd my-tool

# Dependencias comuns para ferramentas CLI
cargo add clap --features derive    # Parsing de argumentos
cargo add serde --features derive   # Serializacao
cargo add serde_json                # Manipulacao de JSON
cargo add anyhow                    # Tratamento de erros
cargo add regex                     # Pattern matching
```

```bash
# Scaffold de pacote npm (para bibliotecas/utilitarios)
mkdir my-package && cd my-package
pnpm init
pnpm install -D typescript tsup vitest
mkdir src
```

**O Fluxo de LLM para Construir**

{? if settings.has_llm ?}
Voce tem uma LLM configurada ({= settings.llm_provider | fallback("local") =} / {= settings.llm_model | fallback("seu modelo") =}). Use-a como seu par de programacao durante o sprint — ela acelera significativamente a geracao de scaffolding e boilerplate.
{? endif ?}

Nao peca para a LLM construir todo o seu produto. Isso produz codigo generico e fragil. Em vez disso:

1. **Voce** escreve a arquitetura: estrutura de arquivos, fluxo de dados, interfaces-chave
2. **LLM** gera boilerplate: componentes repetitivos, funcoes utilitarias, definicoes de tipo
3. **Voce** escreve a logica central: a parte que torna seu produto diferente
4. **LLM** gera testes: testes unitarios, casos extremos, testes de integracao
5. **Voce** revisa e edita tudo: seu nome esta neste produto

Trabalho paralelo enquanto voce coda: abra um segundo chat de LLM e peca para rascunhar o texto da sua landing page, README e documentacao. Voce vai editar esses a noite, mas os primeiros rascunhos estarao prontos.

**Disciplina de Tempo**

```
14:00 — Feature 1 (funcionalidade central): 2 horas
         Se nao estiver funcionando as 16:00, corte escopo.
16:00 — Feature 2 (usabilidade): 1 hora
         Mantenha simples. Lance o polimento depois.
17:00 — Feature 3 (diferencial): 1 hora
         Isso e o que faz voce valer o pagamento. Foque aqui.
18:00 — PARE DE CODAR. Nao precisa ser perfeito.
```

> **Erro Comum:** "So mais uma feature antes de eu parar." E assim que projetos de fim de semana se tornam projetos de um mes. As 3 features sao seu escopo. Se voce pensar em uma grande ideia durante a construcao, escreva na sua lista v0.2 e siga em frente. Voce pode adicionar na proxima semana depois de ter clientes pagantes.

#### Bloco da Noite (2 horas): Escrever a Landing Page

Sua landing page tem um trabalho: convencer um visitante a pagar. Nao precisa ser bonita. Precisa ser clara.

**A Landing Page de 5 Secoes**

Toda landing page bem-sucedida de produto para desenvolvedores segue esta estrutura. Nao reinvente:

```
Secao 1: TITULO + SUBTITULO
  - O que faz em 8 palavras ou menos
  - Para quem e e qual resultado eles obtêm

Secao 2: O PROBLEMA
  - 3 dores que seu cliente-alvo reconhece
  - Use a linguagem exata deles da sua mineracao de threads

Secao 3: A SOLUCAO
  - Screenshots ou exemplos de codigo do seu produto
  - 3 features mapeadas para as 3 dores acima

Secao 4: PRECO
  - Um ou dois tiers. Mantenha simples para v0.1.
  - Opcao de cobranca anual se for assinatura.

Secao 5: CTA (Call to Action)
  - Um botao. "Comece Agora", "Compre Agora", "Baixe".
  - Repita o beneficio principal.
```

**Exemplo Real de Copy — Kit de Dashboard Tailwind:**

```markdown
# Secao 1
## DashKit — Componentes de Dashboard Tailwind Prontos para Producao
Lance seu dashboard SaaS em horas, nao semanas.
12 componentes copy-paste. Dark mode. $29.

# Secao 2
## O Problema
- Kits de UI genericos te dao 500 componentes mas zero coesao
- Construir UIs de dashboard do zero leva 40+ horas
- Opcoes gratis parecem Bootstrap de 2018

# Secao 3
## O Que Voce Recebe
- **12 componentes** projetados para funcionar juntos (nao uma colecao aleatoria)
- **Dark mode** integrado — alterne com um prop
- **Codigo copy-paste** — sem npm install, sem dependencias, sem lock-in
[screenshot dos exemplos de componentes]

# Secao 4
## Precos
**DashKit** — $29 pagamento unico
- Todos os 12 componentes com codigo-fonte
- Atualizacoes gratis por 12 meses
- Use em projetos ilimitados

**DashKit Pro** — $59 pagamento unico
- Tudo no DashKit
- 8 templates de pagina completa (analytics, CRM, admin, configuracoes)
- Arquivos de design no Figma
- Solicitacoes de features prioritarias

# Secao 5
## Lance seu dashboard neste fim de semana.
[Comprar DashKit — $29]
```

**Exemplo Real de Copy — Scrubber de PII em Logs:**

```markdown
# Secao 1
## ScrubLog — Remova PII de Arquivos de Log em Segundos
Conformidade GDPR para seus logs. Um comando.

# Secao 2
## O Problema
- Seus logs contem emails, IPs e nomes que voce nao deveria estar armazenando
- Redacao manual leva horas e deixa passar coisas
- Ferramentas enterprise custam $500/mes e exigem um PhD para configurar

# Secao 3
## Como Funciona
```bash
cat server.log | scrublog > clean.log
```
- Detecta 15+ padroes de PII automaticamente
- Regras customizadas via configuracao YAML
- Lida com formatos JSON, Apache, Nginx e texto plano
[screenshot do terminal mostrando antes/depois]

# Secao 4
## Precos
**Pessoal** — Gratis
- 5 padroes de PII, 1 formato de log

**Pro** — $19/mes
- Todos os 15+ padroes de PII
- Todos os formatos de log
- Regras customizadas
- Compartilhamento de configuracao de equipe

# Secao 5
## Pare de armazenar PII que voce nao precisa.
[Obter ScrubLog Pro — $19/mes]
```

**Fluxo de LLM para Copy:**

1. Alimente a LLM com sua auditoria de concorrentes e resultados da mineracao de threads
2. Peca para rascunhar o texto da landing page usando o template de 5 secoes
3. Edite impiedosamente: substitua cada frase vaga por uma especifica
4. Leia em voz alta. Se alguma frase te faz torcer o nariz, reescreva.

**Construindo a Landing Page:**

Para um sprint de 48 horas, nao construa uma landing page customizada do zero. Use uma dessas opcoes:

{? if stack.contains("react") ?}
- **Seu app React** — Como voce trabalha com React, faca da landing page a homepage deslogada do seu app ou adicione uma rota de marketing no Next.js. Zero custo de troca de contexto.
{? endif ?}
- **O proprio site do seu produto** — Se e um web app, faca da landing page a homepage deslogada
- **Astro + Tailwind** — Site estatico, faz deploy na Vercel em 2 minutos, extremamente rapido
- **Next.js** — Se seu produto ja e React, adicione uma rota de pagina de marketing
- **Framer** (https://framer.com) — Construtor visual, exporta codigo limpo, tier gratis disponivel
- **Carrd** (https://carrd.co) — $19/ano, sites de uma pagina super simples

```bash
# O caminho mais rapido: site estatico Astro
pnpm create astro@latest my-product-site
cd my-product-site
pnpm install
# Adicionar Tailwind
pnpm astro add tailwind
```

Voce deve ter uma landing page com texto pronto ate o final de sabado. Nao precisa de ilustracoes customizadas. Nao precisa de animacoes. Precisa de palavras claras e um botao de compra.

### Dia 2 — Domingo

#### Bloco da Manha (3 horas): Deploy

Seu produto precisa estar no ar na internet em uma URL real. Nao localhost. Nao uma URL de preview da Vercel com um hash aleatorio. Um dominio real, com HTTPS, que voce pode compartilhar e pessoas podem visitar.

**Passo 1: Deploy da Aplicacao (60 minutos)**

{? if computed.os_family == "windows" ?}
Como voce esta no Windows, certifique-se de que o WSL2 esta disponivel se suas ferramentas de deploy exigem. A maioria das ferramentas CLI de deploy (Vercel, Fly.io) funciona nativamente no Windows, mas alguns scripts assumem caminhos Unix.
{? elif computed.os_family == "macos" ?}
No macOS, todas as CLIs de deploy instalam perfeitamente via Homebrew ou download direto. Voce esta no caminho de deploy mais suave.
{? elif computed.os_family == "linux" ?}
No Linux, voce tem o ambiente de deploy mais flexivel. Todas as ferramentas CLI funcionam nativamente, e voce tambem pode hospedar por conta propria na sua maquina se voce tem um IP estatico e quer economizar em custos de hospedagem.
{? endif ?}

Escolha sua plataforma de deploy baseado no que voce construiu:

**Site estatico / SPA (biblioteca de componentes, landing page, site de docs):**
```bash
# Vercel — o caminho mais rapido para sites estaticos e Next.js
pnpm install -g vercel
vercel

# Vai te fazer perguntas. Diga sim para tudo.
# Seu site esta no ar em ~60 segundos.
```

**Web app com backend (ferramenta SaaS, servico de API):**
```bash
# Railway — simples, bom tier gratis, lida com bancos de dados
# https://railway.app
# Conecte seu repo do GitHub e faca deploy.

# Ou Fly.io — mais controle, deploy global na edge
# https://fly.io
curl -L https://fly.io/install.sh | sh
fly launch
fly deploy
```

**Ferramenta CLI / pacote npm:**
```bash
# Registro npm
npm publish

# Ou distribua como binario via GitHub Releases
# Use cargo-dist para projetos Rust
cargo install cargo-dist
cargo dist init
cargo dist build
# Faca upload dos binarios para a release do GitHub
```

**Passo 2: Compre um Dominio (30 minutos)**

Um dominio real custa $12/ano. Se voce nao pode investir $12 no seu negocio, voce nao esta levando a serio ter um negocio.

**Onde comprar:**
- **Namecheap** (https://namecheap.com) — $8-12/ano para .com, bom gerenciamento de DNS
- **Cloudflare Registrar** (https://dash.cloudflare.com) — Preco ao custo (frequentemente $9-10/ano para .com), DNS excelente
- **Porkbun** (https://porkbun.com) — Frequentemente mais barato no primeiro ano, boa UI

**Dicas para nomear o dominio:**
- Mais curto e melhor. 2 silabas ideal, 3 no maximo.
- `.com` ainda ganha em confianca. `.dev` e `.io` sao aceitaveis para ferramentas de desenvolvedor.
- Verifique disponibilidade no seu registrador, nao no GoDaddy (eles fazem front-run nas buscas).
- Nao gaste mais de 15 minutos escolhendo. O nome importa menos do que voce pensa.

```bash
# Aponte seu dominio para a Vercel
# No painel da Vercel: Settings > Domains > Adicione seu dominio
# Depois nas configuracoes de DNS do seu registrador, adicione:
# Registro A: @ -> 76.76.21.21
# Registro CNAME: www -> cname.vercel-dns.com

# Ou se usar Cloudflare para DNS:
# Apenas adicione os mesmos registros no painel de DNS do Cloudflare
# SSL e automatico tanto com Vercel quanto com Cloudflare
```

**Passo 3: Monitoramento Basico (30 minutos)**

Voce precisa saber duas coisas: o site esta no ar, e as pessoas estao visitando.

**Monitoramento de uptime (gratis):**
- **Better Uptime** (https://betteruptime.com) — Tier gratis monitora 10 URLs a cada 3 minutos
- **UptimeRobot** (https://uptimerobot.com) — Tier gratis monitora 50 URLs a cada 5 minutos

```
Configure monitoramento para:
1. A URL da sua landing page
2. O endpoint de health do seu app (se aplicavel)
3. A URL do seu webhook de pagamento (critico — voce precisa saber se pagamentos quebrarem)
```

**Analytics (respeitando privacidade):**

Nao use Google Analytics. Sua audiencia de desenvolvedores bloqueia, e excessivo para um produto novo, e e uma responsabilidade de privacidade.

- **Plausible** (https://plausible.io) — $9/mes, privacidade primeiro, script de uma linha
- **Fathom** (https://usefathom.com) — $14/mes, privacidade primeiro, leve
- **Umami** (https://umami.is) — Gratis e self-hosted, ou $9/mes na nuvem

```html
<!-- Plausible — uma linha no seu <head> -->
<script defer data-domain="seudominio.com"
  src="https://plausible.io/js/script.js"></script>

<!-- Umami — uma linha no seu <head> -->
<script defer
  src="https://sua-instancia-umami.com/script.js"
  data-website-id="seu-website-id"></script>
```

> **Papo Reto:** Sim, $9/mes para analytics em um produto que ainda nao fez dinheiro parece desnecessario. Mas voce nao pode melhorar o que nao pode medir. O primeiro mes de dados de analytics vai te dizer mais sobre seu mercado do que um mes de achismos. Se $9/mes quebra seu orcamento, hospede o Umami gratis no Railway.

#### Bloco da Tarde (2 horas): Configurar Pagamentos

Se seu produto nao pode aceitar dinheiro, e um projeto de hobby. Configurar pagamentos leva menos tempo do que a maioria dos desenvolvedores pensa — cerca de 20-30 minutos para o fluxo basico.

{? if regional.country ?}
> **Processadores de pagamento recomendados para {= regional.country | fallback("seu pais") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy, PayPal") =}. As opcoes abaixo sao globalmente disponiveis, mas verifique se seu processador preferido suporta pagamentos em {= regional.currency | fallback("sua moeda local") =}.
{? endif ?}

**Opcao A: Lemon Squeezy (Recomendado para Produtos Digitais)**

Lemon Squeezy (https://lemonsqueezy.com) cuida de processamento de pagamento, imposto sobre vendas, IVA e entrega digital em uma unica plataforma. E o caminho mais rapido de zero a aceitar pagamentos.

Por que Lemon Squeezy ao inves de Stripe para seu primeiro produto:
- Atua como Merchant of Record — eles cuidam de imposto sobre vendas, IVA e conformidade por voce
- Paginas de checkout integradas — nenhum trabalho de frontend necessario
- Entrega digital integrada — faca upload dos seus arquivos, eles cuidam do acesso
- 5% + $0,50 por transacao (maior que Stripe, mas economiza horas de dor de cabeca com impostos)

Passo a passo da configuracao:
1. Cadastre-se em https://app.lemonsqueezy.com
2. Crie uma Loja (o nome do seu negocio)
3. Adicione um Produto:
   - Nome, descricao, preco
   - Faca upload dos arquivos para entrega digital (se aplicavel)
   - Configure chaves de licenca (se vendendo software)
4. Pegue sua URL de checkout — e isso que seu botao "Comprar" linka
5. Configure um webhook para automacao pos-compra

```javascript
// Handler de webhook Lemon Squeezy (Node.js/Express)
// POST /api/webhooks/lemonsqueezy

import crypto from 'crypto';

const WEBHOOK_SECRET = process.env.LEMONSQUEEZY_WEBHOOK_SECRET;

export async function handleLemonSqueezyWebhook(req, res) {
  // Verificar assinatura do webhook
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

      // Enviar email de boas-vindas, conceder acesso, criar chave de licenca, etc.
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

**Opcao B: Stripe (Mais Controle, Mais Trabalho)**

Stripe (https://stripe.com) te da mais controle mas exige que voce lide com conformidade fiscal separadamente. Melhor para SaaS com faturamento complexo.

```javascript
// Sessao de Checkout Stripe (Node.js)
// Cria uma pagina de checkout hospedada

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
          unit_amount: 5900, // $59.00 em centavos
        },
        quantity: 1,
      },
    ],
    mode: 'payment', // 'subscription' para recorrente
    success_url: `${process.env.DOMAIN}/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${process.env.DOMAIN}/pricing`,
    customer_email: req.body.email, // Pre-preencher se voce tem
  });

  return res.json({ url: session.url });
}

// Handler de webhook Stripe
export async function handleStripeWebhook(req, res) {
  const sig = req.headers['stripe-signature'];

  let event;
  try {
    event = stripe.webhooks.constructEvent(
      req.body, // body cru, nao JSON parseado
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

**Para Ambas as Plataformas — Teste Antes de Lancar:**

```bash
# Lemon Squeezy: Use o modo de teste no painel
# Ative "Test mode" no canto superior direito do painel do Lemon Squeezy
# Use o numero do cartao: 4242 4242 4242 4242, qualquer validade futura, qualquer CVC

# Stripe: Use chaves de API do modo teste
# Cartao de teste: 4242 4242 4242 4242
# Cartao de teste com recusa: 4000 0000 0000 0002
# Cartao de teste exigindo autenticacao: 4000 0025 0000 3155
```

Percorra todo o fluxo de compra voce mesmo no modo teste. Clique no botao de compra, complete o checkout, verifique se o webhook dispara, verifique se o acesso e concedido. Se qualquer etapa falhar no modo teste, vai falhar para clientes reais.

> **Erro Comum:** "Vou configurar os pagamentos depois, depois de conseguir alguns usuarios." Isso e ao contrario. Configurar pagamentos nao e sobre coletar dinheiro hoje — e sobre validar se alguem pagaria. Um produto sem preco e uma ferramenta gratis. Um produto com preco e um teste de negocio. O preco em si faz parte da validacao.

#### Bloco da Noite (3 horas): Lancamento

Seu produto esta no ar. Pagamentos funcionam. A landing page esta clara. Agora voce precisa que humanos vejam.

**A Estrategia de Lancamento Suave**

Nao faca um "grande lancamento" para seu primeiro produto. Grandes lancamentos criam pressao para ser perfeito, e seu v0.1 nao e perfeito. Em vez disso, faca um lancamento suave: compartilhe em alguns lugares, colete feedback, corrija problemas criticos, depois faca o grande lancamento em 1-2 semanas.

**Plataforma de Lancamento 1: Reddit (30 minutos)**

Poste em r/SideProject e um subreddit de nicho relevante ao seu produto.

Template de post no Reddit:

```markdown
Titulo: Eu construi [o que faz] em um fim de semana — [beneficio-chave]

Corpo:
Oi [subreddit],

Eu tenho ficado frustrado com [o problema] ha um tempo, entao construi
[nome do produto] neste fim de semana.

**O que faz:**
- [Feature 1 — o valor central]
- [Feature 2]
- [Feature 3]

**O que o torna diferente de [concorrente]:**
[Um paragrafo honesto sobre seu diferencial]

**Precos:**
[Seja transparente. "$29 pagamento unico" ou "Tier gratis + $19/mes Pro"]

Adoraria feedback. O que estou deixando passar? O que tornaria isso
util para seu fluxo de trabalho?

[Link para o produto]
```

Regras para posts no Reddit:
- Seja genuinamente util, nao vendedor
- Responda a cada comentario (isso nao e opcional)
- Aceite criticas com elegancia — feedback negativo e o tipo mais valioso
- Nao faca astroturfing (upvotes falsos, multiplas contas). Voce sera pego e banido.

**Plataforma de Lancamento 2: Hacker News (30 minutos)**

Se seu produto e tecnico e interessante, poste um Show HN. Na secao "Detalhes tecnicos", mencione seu stack ({= stack.primary | fallback("seu stack principal") =}) e explique por que voce o escolheu — leitores do HN adoram decisoes tecnicas informadas.

Template de Show HN:

```markdown
Titulo: Show HN: [Nome do Produto] – [o que faz em <70 caracteres]

Corpo:
[Nome do produto] e [uma frase explicando o que faz].

Eu construi isso porque [motivacao genuina — qual problema voce estava
resolvendo para si mesmo].

Detalhes tecnicos:
- Construido com [stack]
- [Decisao tecnica interessante e o porque]
- [O que torna a implementacao digna de nota]

Experimente: [URL]

Feedback e bem-vindo. Estou particularmente interessado em [pergunta especifica
para a audiencia do HN].
```

Dicas para o HN:
- Poste entre 7-9 AM Horario do Leste dos EUA (maior trafego)
- O titulo importa mais que tudo. Seja especifico e tecnico.
- Leitores do HN respeitam substancia tecnica mais que polimento de marketing
- Responda a comentarios imediatamente nas primeiras 2 horas. Velocidade de comentarios afeta o ranking.
- Nao implore por upvotes. Apenas poste e interaja.

**Plataforma de Lancamento 3: Twitter/X (30 minutos)**

Escreva uma thread de lancamento build-in-public:

```
Tweet 1 (Gancho):
Eu construi [produto] em 48 horas neste fim de semana.

Ele [resolve problema especifico] para [audiencia especifica].

Aqui esta o que eu lancei, o que aprendi e os numeros reais. Thread:

Tweet 2 (O Problema):
O problema:
[Descreva a dor em 2-3 frases]
[Inclua um screenshot ou exemplo de codigo mostrando a dor]

Tweet 3 (A Solucao):
Entao eu construi [nome do produto].

[Screenshot/GIF do produto em acao]

Ele faz tres coisas:
1. [Feature 1]
2. [Feature 2]
3. [Feature 3]

Tweet 4 (Detalhe Tecnico):
Stack tecnico para os nerds:
- [Frontend]
- [Backend]
- [Hospedagem — mencione a plataforma especifica]
- [Pagamentos — mencione Lemon Squeezy/Stripe]
- Custo total para rodar: $XX/mes

Tweet 5 (Preco):
Precos:
[Preco claro, mesmo da landing page]
[Link para o produto]

Tweet 6 (Pedido):
Adoraria feedback de qualquer pessoa que [descreve usuario-alvo].

O que estou deixando passar? O que tornaria isso imprescindivel para voce?
```

**Plataforma de Lancamento 4: Comunidades Relevantes (30 minutos)**

Identifique 2-3 comunidades onde seu publico-alvo frequenta:

- Servidores Discord (comunidades de desenvolvedores, servidores especificos de frameworks)
- Comunidades Slack (muitas comunidades de dev de nicho tem grupos Slack)
- Dev.to / Hashnode (escreva um post curto "Eu construi isso")
- Indie Hackers (https://indiehackers.com) — especificamente projetado para isso
- Grupos relevantes de Telegram ou WhatsApp

**Primeiras 48 Horas Apos o Lancamento — O Que Observar:**

```
Metricas para acompanhar:
1. Visitantes unicos (do analytics)
2. Taxa de clique da landing page → checkout (deve ser 2-5%)
3. Taxa de conversao checkout → compra (deve ser 1-3%)
4. Taxa de rejeicao (acima de 80% significa que seu titulo/hero esta errado)
5. Fontes de trafego (de onde seus visitantes estao vindo?)
6. Comentarios e feedback (qualitativo — o que as pessoas estao dizendo?)

Matematica de exemplo:
- 500 visitantes em 48 horas (razoavel do Reddit + HN + Twitter)
- 3% clicam "Comprar" = 15 visitas ao checkout
- 10% completam a compra = 1-2 vendas
- A $29/venda = $29-58 no seu primeiro fim de semana

Isso nao e dinheiro de aposentadoria. E dinheiro de VALIDACAO.
$29 de um desconhecido na internet prova que seu produto tem valor.
```

Nao entre em panico se voce tiver zero vendas nas primeiras 48 horas. Olhe para seu funil:
- Zero visitantes? Sua distribuicao e o problema, nao seu produto.
- Visitantes mas zero cliques em "Comprar"? Seu texto ou preco e o problema.
- Cliques em "Comprar" mas zero conclusoes? Seu fluxo de checkout esta quebrado ou seu preco e muito alto para o valor percebido.

Cada um desses tem uma correcao diferente. E por isso que metricas importam.

### Sua Vez

1. **Bloqueie o tempo.** Abra seu calendario agora mesmo e bloqueie o proximo sabado das 8:00 as 20:00 e domingo das 8:00 as 20:00. Rotule como "Sprint de 48 Horas." Trate como um voo que voce nao pode remarcar.

2. **Escolha sua ideia.** Escolha um motor de receita do Modulo R. Escreva o escopo de 3 features para seu v0.1. Se voce nao consegue escolher um, escolha o que voce consegue explicar para um nao-desenvolvedor em uma frase.
{? if dna.primary_stack ?}
   Seu caminho de execucao mais forte e construir algo com {= dna.primary_stack | fallback("seu stack principal") =} — lance mais rapido onde voce ja tem expertise profunda.
{? endif ?}

3. **Pre-trabalho.** Antes de sabado, crie contas em:
   - Vercel, Railway, ou Fly.io (deploy)
   - Lemon Squeezy ou Stripe (pagamentos)
   - Namecheap, Cloudflare, ou Porkbun (dominio)
   - Plausible, Fathom, ou Umami (analytics)
   - Better Uptime ou UptimeRobot (monitoramento)

   Faca isso em um dia de semana a noite para que sabado seja pura construcao, nao criacao de contas.

4. **Prepare suas plataformas de lancamento.** Se voce nao tem uma conta no Reddit com algum karma, comece a participar de subreddits relevantes esta semana. Contas que so postam auto-promocao sao sinalizadas. Se voce nao tem uma conta no Hacker News, crie uma e participe de algumas discussoes primeiro.

---

## Licao 2: A Mentalidade "Lance, Depois Melhore"

*"v0.1 com 3 features vence v1.0 que nunca e lancado."*

### A Armadilha do Perfeccionismo

Desenvolvedores sao unicamente suscetiveis a um modo de falha especifico: construir em privado para sempre. Nos sabemos como e "bom codigo." Sabemos que nosso v0.1 nao e bom codigo. Entao refatoramos. Adicionamos tratamento de erros. Escrevemos mais testes. Melhoramos a arquitetura. Fazemos tudo exceto a unica coisa que importa: mostrar para humanos.

Aqui esta uma verdade que vai te poupar milhares de horas: **seus clientes nao leem seu codigo-fonte.** Eles nao se importam com sua arquitetura. Nao se importam com sua cobertura de testes. Eles se importam com uma coisa: isso resolve meu problema?

Um produto com codigo espaguete que resolve um problema real vai gerar dinheiro. Um produto com arquitetura linda que nao resolve nenhum problema nao vai gerar nada.

Isso nao e uma desculpa para escrever codigo ruim. E uma declaracao de prioridade. Lance primeiro. Refatore depois. A refatoracao sera mais bem informada pelos dados de uso real de qualquer forma.

### Como "Lance, Depois Melhore" Se Desenrola

Considere este cenario: um desenvolvedor lanca um pacote de templates Notion para gerentes de engenharia de software. Aqui esta como fica no lancamento:

- 5 templates (nao 50)
- Uma pagina no Gumroad com um paragrafo de descricao e 3 screenshots
- Nenhum site customizado
- Nenhuma lista de email
- Nenhum seguidor em redes sociais
- Preco: $29

Eles postam no Reddit e Twitter. Essa e toda a estrategia de marketing.

Resultados do mes 1:
- ~170 vendas a $29 = ~$5.000
- Apos a comissao do Gumroad (10%): ~$4.500
- Tempo investido: ~30 horas no total (construindo templates + escrevendo descricoes)
- Taxa horaria efetiva: ~$150/hora

Era "perfeito"? Nao. Os templates tinham inconsistencias de formatacao. Algumas descricoes eram genericas. Os clientes nao se importaram. Eles se importaram que economizou o trabalho de construir os templates eles mesmos.

No mes 3, baseado no feedback dos clientes, o desenvolvedor:
- Corrigiu os problemas de formatacao
- Adicionou mais templates (os que os clientes pediram especificamente)
- Aumentou o preco para $39 (clientes existentes receberam atualizacoes gratis)
- Criou um tier "Pro" com um video passo a passo acompanhante

O produto que eles lancaram era pior em todos os sentidos do que o produto que tinham 90 dias depois. Mas a versao de 90 dias so existiu porque a versao de lancamento gerou o feedback e receita para guiar o desenvolvimento.

> **NOTA:** Para validacao do mundo real do modelo "lance feio, melhore rapido": Josh Comeau fez pre-venda de $550K do seu curso CSS for JavaScript Developers na primeira semana (Fonte: failory.com). Wes Bos gerou $10M+ em vendas totais de cursos para desenvolvedores usando lancamentos iterativos (Fonte: foundershut.com). Ambos comecaram com produtos v1 imperfeitos e iteraram baseados em feedback real de clientes.

### Os Primeiros 10 Clientes Te Dizem Tudo

Seus primeiros 10 clientes pagantes sao as pessoas mais importantes do seu negocio. Nao por causa do dinheiro — 10 vendas a $29 sao $290, o que compra suas compras do mes. Eles sao importantes porque sao voluntarios para sua equipe de desenvolvimento de produto.

O que fazer com seus primeiros 10 clientes:

1. **Envie um email de agradecimento pessoal.** Nao automatizado. Pessoal. "Ei, vi que voce comprou [produto]. Obrigado. Estou desenvolvendo ativamente — existe algo que voce gostaria que fizesse que nao faz?"

2. **Leia cada resposta.** Alguns nao vao responder. Alguns vao responder com "parece otimo, obrigado." Mas 2-3 de 10 vao escrever paragrafos sobre o que querem. Esses paragrafos sao seu roadmap.

3. **Procure padroes.** Se 3 de 10 pessoas pedem a mesma feature, construa. Isso e um sinal de demanda de 30% de clientes pagantes. Nenhuma pesquisa vai te dar dados tao bons.

4. **Pergunte sobre a disposicao deles de pagar mais.** "Estou planejando um tier Pro com [feature X]. Isso valeria $49 para voce?" Direto. Especifico. Te da dados de precificacao.

```
Template de email para os primeiros 10 clientes:

Assunto: Pergunta rapida sobre [nome do produto]

Ei [nome],

Percebi que voce adquiriu [nome do produto] — obrigado por ser
um dos primeiros clientes.

Estou construindo isso ativamente e lancando atualizacoes semanalmente.
Pergunta rapida: qual e a UNICA coisa que voce gostaria que fizesse
que nao faz?

Nao existem respostas erradas. Mesmo se parece um pedido grande,
eu quero ouvir.

Obrigado,
[Seu nome]
```

### Como Lidar com Feedback Negativo

Seu primeiro feedback negativo vai parecer pessoal. Nao e pessoal. E dado.

**Framework para processar feedback negativo:**

```
1. PAUSE. Nao responda por 30 minutos. Sua reacao emocional
   nao e util.

2. CATEGORIZE o feedback:
   a) Relato de bug — corrija. Agradeca.
   b) Pedido de feature — adicione ao backlog. Agradeca.
   c) Reclamacao de preco — anote. Verifique se e um padrao.
   d) Reclamacao de qualidade — investigue. E valida?
   e) Troll/irracional — ignore. Siga em frente.

3. RESPONDA (apenas para a, b, c, d):
   "Obrigado pelo feedback. [Reconheca o problema especifico].
   Estou [corrigindo agora / adicionando ao roadmap / investigando].
   Vou te avisar quando estiver resolvido."

4. AJA. Se voce prometeu corrigir algo, corrija em uma semana.
   Nada constroi lealdade mais rapido do que mostrar aos clientes
   que o feedback deles leva a mudancas reais.
```

> **Papo Reto:** Voce vai receber alguem que diz que seu produto e lixo. Vai doer. Mas se seu produto esta no ar e gerando dinheiro, voce ja fez algo que a maioria dos desenvolvedores nunca faz. A pessoa criticando da secao de comentarios nao lancou nada. Voce lancou. Continue lancando.

### O Ciclo de Iteracao Semanal

Apos o lancamento, seu fluxo de trabalho se torna um loop apertado:

```
Segunda:  Revise as metricas da ultima semana e feedback dos clientes
Terca:    Planeje a melhoria desta semana (UMA coisa, nao cinco)
Quarta:   Construa a melhoria
Quinta:   Teste e faca deploy da melhoria
Sexta:    Escreva um changelog/post de atualizacao
Fim de semana: Marketing — um blog post, um post social, uma interacao em comunidade

Repita.
```

A palavra-chave e UMA melhoria por semana. Nao uma reformulacao de features. Nao um redesign. Uma coisa que torna o produto levemente melhor para seus clientes existentes. Ao longo de 12 semanas, sao 12 melhorias guiadas por dados de uso real. Seu produto apos 12 semanas deste ciclo sera dramaticamente melhor do que qualquer coisa que voce poderia ter projetado em isolamento.

### Receita Valida Mais Rapido Que Pesquisas

Pesquisas mentem. Nao intencionalmente — as pessoas sao apenas ruins em prever seu proprio comportamento. "Voce pagaria $29 por isso?" recebe respostas faceis de "sim." Mas "aqui esta a pagina de checkout, digite seu cartao de credito" recebe respostas honestas.

E por isso que voce lanca com pagamentos desde o dia um:

| Metodo de Validacao | Tempo para Sinal | Qualidade do Sinal |
|---|---|---|
| Pesquisa / enquete | 1-2 semanas | Baixa (pessoas mentem) |
| Landing page com cadastro de email | 1-2 semanas | Media (interesse, nao compromisso) |
| Landing page com preco mas sem checkout | 1 semana | Media-Alta (aceitacao de preco) |
| **Produto ao vivo com checkout real** | **48 horas** | **Mais alta (comportamento real de compra)** |

O preco de $0 nao revela nada. O preco de $29 revela tudo.

### Sua Vez

1. **Escreva seu compromisso de "lancamento feio."** Abra um arquivo de texto e escreva: "Vou lancar [nome do produto] em [data] mesmo que nao esteja perfeito. Escopo do v0.1: [3 features]. Nao vou adicionar a Feature 4 antes do lancamento." Assine (metaforicamente). Consulte quando a vontade de polir atacar.

2. **Rascunhe seu email para os primeiros 10 clientes.** Escreva o template de email de agradecimento pessoal agora, antes de ter clientes. Quando a primeira venda chegar, voce quer enviar dentro da hora.

3. **Configure seu rastreador de iteracoes.** Crie uma planilha simples ou pagina no Notion com colunas: Semana | Melhoria Feita | Impacto na Metrica | Feedback do Cliente. Isso se torna seu registro de decisoes para o que construir em seguida.

---

## Licao 3: Psicologia de Precificacao para Produtos de Desenvolvedor

*"$0 nao e um preco. E uma armadilha."*

### Por Que Gratis E Caro

A verdade mais contraintuitiva em vender produtos para desenvolvedores: **usuarios gratis te custam mais do que clientes pagantes.**

Usuarios gratis:
- Fazem mais solicitacoes de suporte (nao tem pele em jogo)
- Demandam mais features (se sentem no direito porque nao estao pagando)
- Fornecem feedback menos util ("e legal" nao e acionavel)
- Desistem em taxas mais altas (nao ha custo de troca)
- Falam menos do seu produto para outros (coisas gratis tem baixo valor percebido)

Clientes pagantes:
- Estao investidos no seu sucesso (querem que sua compra seja uma boa decisao)
- Fornecem feedback especifico e acionavel (querem que o produto melhore)
- Sao mais faceis de reter (ja decidiram pagar; inercia trabalha a seu favor)
- Indicam outros com mais frequencia (recomendar algo que voce pagou valida sua compra)
- Respeitam seu tempo (entendem que voce esta gerindo um negocio)

A unica razao para oferecer um tier gratis e como mecanismo de geracao de leads para o tier pago. Se seu tier gratis e bom o suficiente para que as pessoas nunca facam upgrade, voce nao tem um tier gratis — voce tem um produto gratis com um botao de doacao.

> **Erro Comum:** "Vou tornar gratis para conseguir usuarios primeiro, depois cobrar." Isso quase nunca funciona. Os usuarios que voce atrai a $0 esperam $0 para sempre. Quando voce adiciona um preco, eles vao embora. Os usuarios que teriam pago $29 desde o dia um nunca encontraram seu produto porque voce o posicionou como uma ferramenta gratis. Voce atraiu a audiencia errada.

{@ insight cost_projection @}

### Os Tiers de Precificacao de Produtos para Desenvolvedores

Apos analisar centenas de produtos de desenvolvedor bem-sucedidos, esses pontos de preco funcionam consistentemente. Todos os precos abaixo estao em USD — se voce esta precificando em {= regional.currency | fallback("sua moeda local") =}, ajuste para o poder de compra local e normas de mercado.

**Tier 1: $9-29 — Ferramentas e Utilitarios para Desenvolvedores**

Produtos nessa faixa resolvem um problema especifico e estreito. Uma unica compra, use hoje.

```
Exemplos:
- Extensao do VS Code com features premium: $9-15
- Ferramenta CLI com features pro: $15-19
- Ferramenta SaaS de proposito unico: $9-19/mes
- Pequena biblioteca de componentes: $19-29
- Extensao de DevTools do navegador: $9-15

Psicologia do comprador: Territorio de compra impulsiva. O desenvolvedor ve,
reconhece o problema, compra sem perguntar ao gerente.
Sem necessidade de aprovacao de orcamento. Cartao de credito → pronto.

Insight-chave: Neste preco, sua landing page precisa converter em
menos de 2 minutos. O comprador nao vai ler uma lista longa de features.
Mostre o problema, mostre a solucao, mostre o preco.
```

**Tier 2: $49-99 — Templates, Kits e Ferramentas Abrangentes**

Produtos nessa faixa economizam tempo significativo. Multiplos componentes funcionando juntos.

```
Exemplos:
- Kit completo de templates de UI: $49-79
- Boilerplate SaaS com auth, faturamento, dashboards: $79-99
- Conjunto abrangente de icones/ilustracoes: $49-69
- Toolkit CLI multiproposito: $49
- Biblioteca wrapper de API com docs extensivos: $49-79

Psicologia do comprador: Compra considerada. O desenvolvedor avalia por
5-10 minutos. Compara com alternativas. Calcula tempo economizado.
"Se isso me economiza 10 horas e eu valorizo meu tempo a $50/hora,
$79 e uma decisao obvia."

Insight-chave: Voce precisa de um ponto de comparacao. Mostre o
tempo/esforco para construir isso do zero vs. comprar seu kit.
Inclua depoimentos se voce tem.
```

**Tier 3: $149-499 — Cursos, Solucoes Abrangentes, Templates Premium**

Produtos nessa faixa transformam uma habilidade ou fornecem um sistema completo.

```
Exemplos:
- Curso em video (10+ horas): $149-299
- Kit starter SaaS com codigo completo + video passo a passo: $199-299
- Biblioteca de componentes enterprise: $299-499
- Toolkit abrangente de desenvolvedor (multiplas ferramentas): $199
- "Construa X do Zero" codebase completo + licoes: $149-249

Psicologia do comprador: Compra de investimento. O comprador precisa
justificar o gasto (para si mesmo ou para o gerente). Precisa de prova
social, previews detalhados e uma narrativa clara de ROI.

Insight-chave: Neste tier, ofereca garantia de devolucao do dinheiro.
Reduz a ansiedade de compra e aumenta conversoes. Taxas de reembolso
para produtos digitais de desenvolvedor sao tipicamente 3-5%.
O aumento nas conversoes supera em muito os reembolsos.
```

### A Estrategia de Precificacao de 3 Tiers

Se seu produto suporta, ofereca tres tiers de preco. Isso nao e aleatorio — explora um vies cognitivo bem documentado chamado "efeito de palco central." Quando apresentadas com tres opcoes, a maioria das pessoas escolhe a do meio.

```
Estrutura de tiers:

BASICO          PRO (destacado)       EQUIPE/ENTERPRISE
$29             $59                   $149
Features core   Tudo no Basico        Tudo no Pro
                + features premium    + features de equipe
                + suporte prioritario + licenca comercial

Distribuicao de conversao (tipica):
- Basico: 20-30%
- Pro: 50-60% ← este e seu alvo
- Equipe: 10-20%
```

**Como projetar os tiers:**

1. Comece com o tier **Pro**. Este e o produto que voce realmente quer vender, no preco que reflete seu valor. Projete este primeiro.

2. Crie o tier **Basico** removendo features do Pro. Remova o suficiente para que Basico resolva o problema mas Pro o resolva *bem*. Basico deve parecer levemente frustrante — usavel, mas claramente limitado.

3. Crie o tier **Equipe** adicionando features ao Pro. Licenciamento multi-assento, direitos de uso comercial, suporte prioritario, branding customizado, acesso ao codigo-fonte, arquivos Figma, etc.

**Exemplo real de pagina de precos:**

```
DashKit

STARTER — $29                    PRO — $59                        EQUIPE — $149
                                 ★ Mais Popular                   Melhor para agencias

✓ 12 componentes core            ✓ Tudo no Starter                ✓ Tudo no Pro
✓ React + TypeScript             ✓ 8 templates de pagina completa ✓ Ate 5 membros de equipe
✓ Dark mode                      ✓ Arquivos de design no Figma    ✓ Licenca comercial
✓ npm install                    ✓ Tabela de dados avancada         (projetos de cliente ilimitados)
✓ 6 meses de atualizacoes       ✓ Integracao com biblioteca      ✓ Suporte prioritario
                                   de graficos                    ✓ Atualizacoes vitalícias
                                 ✓ 12 meses de atualizacoes      ✓ Opcoes de branding customizado
                                 ✓ Solicitacoes de features
                                   prioritarias

[Obter Starter]                  [Obter Pro]                      [Obter Equipe]
```

### Ancoragem de Precos

Ancoragem e o vies cognitivo onde o primeiro numero que as pessoas veem influencia a percepcao dos numeros seguintes. Use eticamente:

1. **Mostre a opcao cara primeiro** (a direita em layouts ocidentais). Ver $149 faz $59 parecer razoavel.

2. **Mostre calculos de "horas economizadas".**
   ```
   "Construir esses componentes do zero leva ~40 horas.
   A $50/hora, sao $2.000 do seu tempo.
   DashKit Pro: $59."
   ```

3. **Use reformulacao "por dia" para assinaturas.**
   ```
   "$19/mes" → "Menos de $0,63/dia"
   "$99/ano" → "$8,25/mes" ou "$0,27/dia"
   ```

4. **Desconto de cobranca anual.** Ofereca 2 meses gratis nos planos anuais. Isso e padrao e esperado. Cobranca anual reduz churn em 30-40% porque o cancelamento requer uma decisao consciente em um unico ponto de renovacao, nao uma decisao mensal continua.

```
Mensal: $19/mes
Anual: $190/ano (economize $38 — 2 meses gratis)

Exibir como:
Mensal: $19/mes
Anual: $15,83/mes (cobrado anualmente a $190)
```

### Teste A/B de Precos

Testar precos e valioso mas complicado. Aqui esta como fazer sem ser desonesto:

**Abordagens aceitaveis:**
- Testar precos diferentes em canais de lancamento diferentes (Reddit recebe $29, Product Hunt recebe $39, veja qual converte melhor)
- Mudar seu preco apos 2 semanas e comparar taxas de conversao
- Oferecer um desconto de lancamento ("$29 esta semana, $39 depois") e ver se a urgencia muda o comportamento
- Testar estruturas de tiers diferentes (2 tiers vs 3 tiers) em periodos diferentes

**Nao aceitavel:**
- Mostrar precos diferentes para visitantes diferentes na mesma pagina ao mesmo tempo (discriminacao de precos, erode confianca)
- Cobrar mais baseado em localizacao ou deteccao de navegador (as pessoas conversam, e voce sera pego)

### Quando Aumentar Precos

Aumente seus precos quando qualquer um desses for verdadeiro:

1. **Taxa de conversao esta acima de 5%.** Voce esta muito barato. Uma taxa de conversao saudavel para uma landing page de produto de desenvolvedor e 1-3%. Acima de 5% significa que quase todos que veem o preco concordam que e um bom negocio — o que significa que voce esta deixando dinheiro na mesa.

2. **Ninguem reclamou do preco.** Se zero pessoas de 100 dizem que e muito caro, esta muito barato. Um produto saudavel tem cerca de 20% dos visitantes achando o preco alto. Isso significa que 80% acham justo ou um bom negocio.

3. **Voce adicionou features significativas desde o lancamento.** Voce lancou a $29 com 3 features. Voce agora tem 8 features e melhor documentacao. O produto vale mais. Cobre mais.

4. **Voce tem depoimentos e prova social.** O valor percebido aumenta com prova social. Uma vez que voce tem 5+ reviews positivos, seu produto vale mais na mente do comprador.

**Como aumentar precos:**
- Anuncie o aumento de preco 1-2 semanas antes ("Preco subindo de $29 para $39 em [data]")
- Mantenha clientes existentes no preco antigo
- Isso nao e mau carater — e pratica padrao e tambem cria urgencia para os indecisos

> **Papo Reto:** A maioria dos desenvolvedores subprecifica em 50-200%. Seu produto de {= regional.currency_symbol | fallback("$") =}29 provavelmente vale {= regional.currency_symbol | fallback("$") =}49. Seu produto de {= regional.currency_symbol | fallback("$") =}49 provavelmente vale {= regional.currency_symbol | fallback("$") =}79. Eu sei disso porque desenvolvedores ancoram na propria disposicao de pagar (baixa — somos mao-de-vaca com ferramentas) ao inves da disposicao do cliente de pagar (mais alta — eles estao comprando uma solucao para um problema que custa tempo). Aumente seus precos mais cedo do que voce pensa.

### Sua Vez

1. **Precifique seu produto.** Baseado na analise de tiers acima, escolha um ponto de preco para seu lancamento v0.1. Escreva. Se voce se sente desconfortavel porque parece "muito alto," voce provavelmente esta na faixa certa. Se parece confortavel, adicione 50%.

2. **Projete sua pagina de precos.** Usando o template de 3 tiers, projete o texto da sua pagina de precos. Identifique quais features vao em cada tier. Identifique seu tier "destacado" (o que voce quer que a maioria compre).

3. **Calcule sua matematica.** Preencha:
   - Preco por venda: {= regional.currency_symbol | fallback("$") =}___
   - Receita mensal alvo: {= regional.currency_symbol | fallback("$") =}___
   - Numero de vendas necessarias por mes: ___
   - Visitantes estimados necessarios na landing page (a 2% de conversao): ___
   - Essa contagem de visitantes e alcancavel com seu plano de distribuicao? (Sim/Nao)

---

## Licao 4: Configuracao Legal Minima Viavel

*"30 minutos de configuracao legal agora economizam 30 horas de panico depois."*

### A Verdade Honesta Sobre Configuracao Legal

A maioria dos desenvolvedores ignora o aspecto legal completamente (arriscado) ou fica paralisada por ele (desperdicado). A abordagem certa e uma configuracao legal minima viavel: protecao suficiente para operar legitimamente, sem gastar $5.000 em um advogado antes de ter feito $5.

Aqui esta o que voce realmente precisa antes da sua primeira venda, o que precisa antes da sua centesima venda, e o que nao precisa ate muito depois.

### Antes da Sua Primeira Venda (Faca Neste Fim de Semana)

**1. Verifique Seu Contrato de Trabalho (30 minutos)**

Se voce tem um emprego CLT ou PJ, leia a clausula de PI do seu contrato de trabalho antes de construir qualquer coisa. Especificamente procure por:

- **Clausulas de cessao de invencoes:** Alguns contratos dizem que tudo que voce cria enquanto empregado — inclusive no seu proprio tempo — pertence ao empregador.
- **Clausulas de nao-concorrencia:** Alguns restringem voce de trabalhar na mesma industria, mesmo como projeto paralelo.
- **Politicas de moonlighting:** Alguns exigem aprovacao por escrito para atividades comerciais externas.

```
O que voce esta procurando:

SEGURO: "Invencoes feitas no tempo da empresa ou usando recursos da
empresa pertencem a empresa." → Seu projeto de fim de semana na sua
maquina pessoal e seu.

NEBULOSO: "Todas as invencoes relacionadas ao negocio atual ou
previsto da empresa." → Se seu projeto paralelo e no mesmo
dominio que seu empregador, busque assessoria juridica.

RESTRITIVO: "Todas as invencoes concebidas durante o periodo de
emprego pertencem a empresa." → Isso e agressivo mas
comum em algumas empresas. Busque assessoria juridica antes de prosseguir.
```

Estados como California, Delaware, Illinois, Minnesota, Washington e outros tem leis que limitam quao amplamente empregadores podem reivindicar suas invencoes pessoais. Mas a linguagem especifica do seu contrato importa.

> **Erro Comum:** "Vou manter em segredo." Se seu produto ficar bem-sucedido o suficiente para importar, alguem vai notar. Se violar seu contrato de trabalho, voce pode perder o produto E seu emprego. 30 minutos lendo seu contrato agora previne isso.

**2. Politica de Privacidade (15 minutos)**

Se seu produto coleta qualquer dado — mesmo apenas um endereco de email para compra — voce precisa de uma politica de privacidade. Isso e uma exigencia legal na UE (GDPR), California (CCPA), e cada vez mais em todos os lugares.

Nao escreva uma do zero. Use um gerador:

- **Termly** (https://termly.io/products/privacy-policy-generator/) — Tier gratis, responda perguntas, receba uma politica
- **Avodocs** (https://www.avodocs.com) — Gratis, templates legais open-source
- **Iubenda** (https://www.iubenda.com) — Tier gratis, gera automaticamente baseado no seu stack tecnico

Sua politica de privacidade deve cobrir:

```markdown
# Politica de Privacidade do [Nome do Produto]
Ultima atualizacao: [Data]

## O Que Coletamos
- Endereco de email (para confirmacao de compra e atualizacoes do produto)
- Informacoes de pagamento (processadas por [Lemon Squeezy/Stripe],
  nunca vemos ou armazenamos seus dados de cartao)
- Analytics basico de uso (visualizacoes de pagina, uso de features — via
  [Plausible/Fathom/Umami], respeitando privacidade, sem cookies)

## O Que NAO Coletamos
- Nao rastreamos voce pela web
- Nao vendemos seus dados para ninguem
- Nao usamos cookies de publicidade

## Como Usamos Seus Dados
- Para entregar o produto que voce comprou
- Para enviar atualizacoes de produto e avisos importantes
- Para melhorar o produto baseado em padroes agregados de uso

## Armazenamento de Dados
- Seus dados sao armazenados em servidores do [provedor de hospedagem] em [regiao]
- Dados de pagamento sao tratados inteiramente pelo [Lemon Squeezy/Stripe]

## Seus Direitos
- Voce pode solicitar uma copia dos seus dados a qualquer momento
- Voce pode solicitar exclusao dos seus dados a qualquer momento
- Contato: [seu email]

## Alteracoes
- Notificaremos voce sobre mudancas significativas via email
```

Coloque isso em `seudominio.com/privacy`. Linke no rodape da sua pagina de checkout.

**3. Termos de Servico (15 minutos)**

Seus termos de servico protegem voce de reivindicacoes irrazoaveis. Para um produto digital, sao diretos.

```markdown
# Termos de Servico do [Nome do Produto]
Ultima atualizacao: [Data]

## Licenca
Quando voce compra [Nome do Produto], voce recebe uma licenca para usa-lo
para fins [pessoais/comerciais].

- **Licenca individual:** Use em seus proprios projetos (ilimitados)
- **Licenca de equipe:** Uso por ate [N] membros de equipe
- Voce NAO pode redistribuir, revender ou compartilhar credenciais de acesso

## Reembolsos
- Produtos digitais: garantia de devolucao de [30 dias / 14 dias]
- Se voce nao esta satisfeito, envie email para [seu email] para reembolso total
- Sem perguntas dentro da janela de reembolso

## Responsabilidade
- [Nome do Produto] e fornecido "como esta" sem garantia
- Nao somos responsaveis por danos decorrentes do uso do produto
- Responsabilidade maxima e limitada ao valor que voce pagou

## Suporte
- Suporte e fornecido via email em [seu email]
- Buscamos responder em [48 horas / 2 dias uteis]

## Modificacoes
- Podemos atualizar estes termos com aviso previo
- Uso continuado constitui aceitacao dos termos atualizados
```

Coloque isso em `seudominio.com/terms`. Linke no rodape da sua pagina de checkout.

### Antes da Sua Centesima Venda (Primeiros Meses)

**4. Entidade Empresarial (1-3 horas + tempo de processamento)**

Operar como pessoa fisica autonoma (o padrao quando voce vende coisas sem formar uma empresa) funciona para suas primeiras vendas. Mas conforme a receita cresce, voce quer protecao de responsabilidade e vantagens tributarias.

{? if regional.country ?}
> **Para {= regional.country | fallback("sua regiao") =}:** O tipo de entidade recomendado e um(a) **{= regional.business_entity_type | fallback("LLC ou equivalente") =}**, com custos tipicos de registro de {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Encontre a secao do seu pais abaixo para orientacao especifica.
{? endif ?}

**Estados Unidos — LLC:**

Uma LLC (Limited Liability Company) e a escolha padrao para negocios solo de desenvolvedor.

```
Custo: $50-500 dependendo do estado (taxa de registro)
Tempo: 1-4 semanas para processamento
Onde registrar: Seu estado de residencia, a menos que haja uma razao especifica
para usar Delaware ou Wyoming

Registro DIY (mais barato):
1. Va ao site do Secretary of State do seu estado
2. Registre "Articles of Organization" (o formulario geralmente e de 1-2 paginas)
3. Pague a taxa de registro ($50-250 dependendo do estado)
4. Obtenha seu EIN (ID fiscal) no IRS.gov — gratis, instantaneo online

Comparacao de estados para desenvolvedores solo:
- Wyoming: $100 registro, $60/ano relatorio anual. Sem imposto de renda estadual.
             Bom para privacidade (nao exige informacoes publicas dos membros).
- Delaware: $90 registro, $300/ano imposto anual. Popular mas nao
            necessariamente melhor para desenvolvedores solo.
- New Mexico: $50 registro, sem relatorio anual. Mais barato para manter.
- California: $70 registro, $800/ano taxa minima de franquia.
              Caro. Voce paga isso mesmo que fature $0.
```

**Stripe Atlas (se voce quer que facam por voce):**

Stripe Atlas (https://atlas.stripe.com) custa $500 e configura uma LLC em Delaware, conta bancaria nos EUA (via Mercury), conta Stripe, e fornece guias fiscais e legais. Se voce nao e dos EUA ou apenas quer que outra pessoa cuide da papelada, vale os $500.

**Reino Unido — Ltd Company:**

```
Custo: GBP 12 no Companies House (https://www.gov.uk/set-up-limited-company)
Tempo: Geralmente 24-48 horas
Continuidade: Declaracao de confirmacao anual (GBP 13), envio anual de contas

Para desenvolvedores solo: Uma Ltd company te da protecao de responsabilidade
e eficiencia tributaria quando lucros excedem ~GBP 50.000/ano.
Abaixo disso, sole trader e mais simples.
```

**Uniao Europeia:**

Cada pais tem sua propria estrutura. Opcoes comuns:
- **Alemanha:** GmbH (caro para configurar) ou registro de freelancer (barato)
- **Holanda:** BV ou eenmanszaak (empreendimento individual)
- **Franca:** auto-entrepreneur (micro-empresa) — muito comum para desenvolvedores solo, imposto fixo simples
- **Estonia:** e-Residencia + OUe estoniana (popular com nomades digitais, empresa completa da UE por ~EUR 190)

**Australia:**

```
Sole trader: Gratis para registrar via aplicacao ABN (https://www.abr.gov.au)
Empresa (Pty Ltd): AUD 538 registro com ASIC
Para desenvolvedores solo: Comece como sole trader. Registre uma empresa
quando a receita justificar a sobrecarga contabil (~AUD 100K+/ano).
```

**5. Obrigacoes Tributarias**

Se voce esta usando Lemon Squeezy como sua plataforma de pagamento, eles cuidam do imposto sobre vendas e IVA como Merchant of Record. Isso e uma simplificacao enorme.

Se voce esta usando Stripe diretamente, voce e responsavel por:
- **Imposto sobre vendas dos EUA:** Varia por estado. Use Stripe Tax ($0,50/transacao) ou TaxJar para automatizar.
- **IVA da UE:** 20-27% dependendo do pais. Exigido para vendas digitais a clientes da UE independentemente de onde voce esta baseado. Lemon Squeezy cuida disso; Stripe Tax pode automatizar.
- **IVA do Reino Unido:** 20%. Exigido se suas vendas no UK excederem GBP 85.000/ano.
- **Impostos sobre Servicos Digitais:** Varios paises impondo. Mais uma razao para usar Lemon Squeezy ate que seu volume justifique gerenciar isso voce mesmo.

{? if regional.country ?}
> **Nota fiscal para {= regional.country | fallback("sua regiao") =}:** {= regional.tax_note | fallback("Consulte um profissional de contabilidade local para especificidades sobre suas obrigacoes.") =}
{? endif ?}

> **Papo Reto:** A maior vantagem do Lemon Squeezy sobre o Stripe para um desenvolvedor solo nao e a pagina de checkout ou as features. E que eles cuidam da conformidade fiscal globalmente. Imposto sobre vendas internacionais e um pesadelo. Lemon Squeezy cobra 5% + $0,50 por transacao e faz o pesadelo desaparecer. Ate voce estar fazendo {= regional.currency_symbol | fallback("$") =}5.000+/mes, os 5% valem a pena. Depois disso, avalie se gerenciar impostos voce mesmo com Stripe + TaxJar economiza dinheiro e sanidade.

**6. Basicos de Propriedade Intelectual**

O que voce precisa saber:

- **Seu codigo e automaticamente protegido por direitos autorais** no momento em que voce o escreve. Nenhum registro necessario. Mas registro (EUA: $65 em copyright.gov) te da posicao legal mais forte em disputas.
- **O nome do seu produto pode ser registrado como marca.** Nao e necessario para o lancamento, mas considere se o produto decolar. Registro de marca nos EUA: $250-350 por classe.
- **Licencas open-source nas suas dependencias importam.** Se voce usa codigo com licenca MIT, tudo certo. Se voce usa codigo com licenca GPL em um produto comercial, voce pode precisar abrir o codigo do seu produto. Verifique as licencas das suas dependencias antes de vender.

```bash
# Verifique as licencas de dependencias do seu projeto (Node.js)
npx license-checker --summary

# Verifique licencas problematicas especificamente
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"

# Para projetos Rust
cargo install cargo-license
cargo license
```

**7. Seguro**

Voce nao precisa de seguro para uma biblioteca de componentes de $29. Voce precisa de seguro se:
- Voce esta prestando servicos (consultoria, processamento de dados) onde erros podem causar perdas ao cliente
- Seu produto lida com dados sensiveis (saude, financeiro)
- Voce esta assinando contratos com clientes enterprise (eles vao exigir)

Quando voce precisar, seguro de responsabilidade profissional (erros e omissoes / E&O) custa $500-1.500/ano para um negocio de desenvolvedor solo.

### Sua Vez

1. **Leia seu contrato de trabalho.** Se voce e empregado, encontre a clausula de PI e a clausula de nao-concorrencia. Categorize-as: Seguro / Nebuloso / Restritivo. Se Nebuloso ou Restritivo, consulte um advogado trabalhista antes de lancar (muitos oferecem consultas gratis de 30 minutos).

2. **Gere seus documentos legais.** Va ao Termly ou Avodocs e gere uma politica de privacidade e termos de servico para seu produto. Salve como HTML ou Markdown. Faca deploy em `/privacy` e `/terms` no dominio do seu produto.

3. **Tome sua decisao de entidade.** Baseado na orientacao acima e sua residencia em {= regional.country | fallback("seu pais") =}, decida: lancar como pessoa fisica (mais rapido) ou formar um(a) {= regional.business_entity_type | fallback("LLC/Ltd/equivalente") =} primeiro (mais protecao). Escreva sua decisao e cronograma.

4. **Verifique suas dependencias.** Execute o verificador de licencas no seu projeto. Resolva qualquer dependencia GPL/AGPL antes de vender um produto comercial.

---

## Licao 5: Canais de Distribuicao Que Funcionam em 2026

*"Construir e 20% do trabalho. Colocar na frente das pessoas e os outros 80%."*

### A Realidade da Distribuicao

A maioria dos produtos para desenvolvedores falha nao porque sao ruins, mas porque ninguem sabe que existem. Distribuicao — colocar seu produto na frente de clientes potenciais — e a habilidade em que a maioria dos desenvolvedores e mais fraca. E e a habilidade que mais importa.

Aqui estao sete canais de distribuicao classificados por esforco, cronograma e retorno esperado. Voce nao precisa de todos os sete. Escolha 2-3 que combinem com seus pontos fortes e sua audiencia.

### Canal 1: Hacker News

**Esforco:** Alto | **Cronograma:** Instantaneo (0-48 horas) | **Natureza:** Tudo-ou-nada

Hacker News (https://news.ycombinator.com) e o canal de distribuicao de evento unico de maior alavancagem para produtos de desenvolvedor. Um post Show HN na primeira pagina pode enviar 5.000-30.000 visitantes em 24 horas. Mas e imprevisivel — a maioria dos posts nao tem tracao nenhuma.

**O que funciona no HN:**
- Produtos tecnicos com detalhes de implementacao interessantes
- Ferramentas focadas em privacidade (a audiencia do HN se importa profundamente com privacidade)
- Ferramentas open-source com um tier pago
- Solucoes inovadoras para problemas conhecidos
- Produtos com demos ao vivo

**O que nao funciona no HN:**
- Lancamentos pesados em marketing ("Revolucionario com IA...")
- Produtos que sao wrappers de outros produtos sem valor original
- Qualquer coisa que pareca um anuncio

**O Playbook do Show HN:**

```
ANTES DE POSTAR:
1. Estude posts Show HN bem-sucedidos recentes na sua categoria
   https://hn.algolia.com — filtre por "Show HN", ordene por pontos
2. Prepare o titulo do seu post: "Show HN: [Nome] – [o que faz, <70 caracteres]"
   Bom: "Show HN: ScrubLog – Strip PII from Log Files in One Command"
   Ruim: "Show HN: Introducing ScrubLog, the AI-Powered Log Anonymization Platform"
3. Tenha uma demo ao vivo pronta (leitores do HN querem testar, nao ler sobre)
4. Prepare respostas para perguntas provaveis (decisoes tecnicas, justificativa de preco)

POSTANDO:
5. Poste entre 7-9 AM Horario do Leste dos EUA, terca a quinta
   (maior trafego, maior chance de tracao)
6. O corpo do seu post deve ter 4-6 paragrafos:
   - O que e (1 paragrafo)
   - Por que voce construiu (1 paragrafo)
   - Detalhes tecnicos (1-2 paragrafos)
   - O que voce esta procurando (feedback, perguntas especificas)

APOS POSTAR:
7. Fique online por 4 horas apos postar. Responda a CADA comentario.
8. Seja humilde e tecnico. O HN recompensa honestidade sobre limitacoes.
9. Se alguem encontrar um bug, corrija ao vivo e responda "Corrigido, obrigado."
10. Nao peca a amigos para dar upvote. O HN tem deteccao de aneis de votos.
```

**Resultados esperados (realistas):**
- 70% dos posts Show HN: <10 pontos, <500 visitantes
- 20% dos posts Show HN: 10-50 pontos, 500-3.000 visitantes
- 10% dos posts Show HN: 50+ pontos, 3.000-30.000 visitantes

E uma loteria com probabilidades carregadas por esforco. Um otimo produto com um otimo post tem talvez 30% de chance de tracao significativa. Nao garantido. Mas o lado positivo e enorme.

### Canal 2: Reddit

**Esforco:** Medio | **Cronograma:** 1-7 dias | **Natureza:** Sustentavel, repetivel

Reddit e o canal de distribuicao mais consistente para produtos de desenvolvedor. Diferente do HN (uma chance), Reddit tem centenas de subreddits de nicho onde seu produto e relevante.

**Selecao de subreddit:**

```
Subreddits gerais de desenvolvedor:
- r/SideProject (140K+ membros) — feito para isso
- r/webdev (2.4M membros) — enorme, competitivo
- r/programming (6.3M membros) — muito competitivo, focado em noticias
- r/selfhosted (400K+ membros) — se seu produto e self-hostavel

Especificos de framework/linguagem:
- r/reactjs, r/nextjs, r/sveltejs, r/vuejs — para ferramentas frontend
- r/rust, r/golang, r/python — para ferramentas especificas de linguagem
- r/node — para ferramentas e pacotes Node.js

Especificos de dominio:
- r/devops — para ferramentas de infraestrutura/deploy
- r/machinelearning — para ferramentas de IA/ML
- r/datascience — para ferramentas de dados
- r/sysadmin — para ferramentas de admin/monitoramento

A cauda longa:
- Pesquise subreddits relacionados ao seu nicho especifico
- Subreddits menores (10K-50K membros) frequentemente tem melhores
  taxas de conversao do que os enormes
```

**Regras de engajamento no Reddit:**

1. **Tenha um historico real no Reddit** antes de postar seu produto. Contas que so postam auto-promocao sao sinalizadas e shadowbanidas.
2. **Siga as regras de cada subreddit** sobre auto-promocao. A maioria permite desde que voce seja um membro contribuinte.
3. **Interaja genuinamente.** Responda perguntas, forneca valor, seja util nos comentarios de outros posts. Depois compartilhe seu produto.
4. **Poste em horarios diferentes** para subreddits diferentes. Verifique https://later.com/reddit ou ferramentas similares para horarios de pico de atividade.

**Resultados esperados (realistas):**
- Post em r/SideProject: 20-100 upvotes, 200-2.000 visitantes
- Subreddit de nicho (50K membros): 10-50 upvotes, 100-1.000 visitantes
- Primeira pagina do r/webdev: 100-500 upvotes, 2.000-10.000 visitantes

### Canal 3: Twitter/X

**Esforco:** Medio | **Cronograma:** 2-4 semanas para ganhar momentum | **Natureza:** Compoe ao longo do tempo

Twitter e um canal de construcao lenta. Seu primeiro tweet de lancamento vai receber 5 curtidas dos seus amigos. Mas se voce compartilha consistentemente seu processo de construcao, sua audiencia compoe.

**A Estrategia Build-in-Public:**

```
Semana 1: Comece a compartilhar seu processo de construcao (antes do lancamento)
- "Trabalhando em um [tipo de produto]. Aqui esta o problema que estou resolvendo: [screenshot]"
- "Dia 3 construindo [produto]. Consegui [feature] funcionando: [GIF/screenshot]"

Semana 2: Compartilhe insights tecnicos da construcao
- "TIL voce precisa [licao tecnica] ao construir [tipo de produto]"
- "Decisao de arquitetura: escolhi [X] ao inves de [Y] porque [razao]"

Semana 3: Lancamento
- Thread de lancamento (formato da Licao 1)
- Compartilhe metricas especificas: "Dia 1: X visitantes, Y cadastros"

Semana 4+: Continuidade
- Compartilhe feedback de clientes (com permissao)
- Compartilhe marcos de receita (pessoas adoram numeros reais)
- Compartilhe desafios e como voce os resolveu
```

**Com quem interagir:**
- Siga e interaja com desenvolvedores no seu nicho
- Responda a tweets de contas maiores com comentarios pensados (nao auto-promocao)
- Participe de Twitter Spaces sobre seu topico
- Cite tweets relevantes com sua perspectiva

**Resultados esperados (realistas):**
- 0-500 seguidores: Tweets de lancamento recebem 5-20 curtidas, <100 visitantes
- 500-2.000 seguidores: Tweets de lancamento recebem 20-100 curtidas, 100-500 visitantes
- 2.000-10.000 seguidores: Tweets de lancamento recebem 100-500 curtidas, 500-5.000 visitantes

Twitter e um investimento de 6 meses, nao uma estrategia de dia de lancamento. Comece agora, mesmo antes do seu produto estar pronto.

### Canal 4: Product Hunt

**Esforco:** Alto | **Cronograma:** 1 dia de atividade intensa | **Natureza:** Impulso unico

Product Hunt (https://producthunt.com) e uma plataforma dedicada de lancamento. Um top-5 diario pode enviar 3.000-15.000 visitantes. Mas exige preparacao.

**Checklist de Lancamento no Product Hunt:**

```
2 SEMANAS ANTES:
- [ ] Crie um perfil de maker no Product Hunt
- [ ] Monte sua listagem no PH: tagline, descricao, imagens, video
- [ ] Prepare 4-5 screenshots/GIFs de alta qualidade
- [ ] Escreva um "primeiro comentario" que explique sua motivacao
- [ ] Alinhe 10-20 pessoas para apoiar no dia do lancamento (nao votos falsos —
      pessoas reais que vao testar o produto e deixar comentarios genuinos)
- [ ] Encontre um "hunter" (alguem com grande audiencia no PH para submeter seu produto)
      ou submeta voce mesmo

DIA DO LANCAMENTO (00:01 Horario do Pacifico):
- [ ] Esteja online a partir da meia-noite PT. O PH reseta a meia-noite.
- [ ] Poste seu "primeiro comentario" imediatamente
- [ ] Compartilhe o link do PH no Twitter, LinkedIn, email, Discord
- [ ] Responda a CADA comentario na sua listagem do PH
- [ ] Poste atualizacoes ao longo do dia ("Acabei de lancar uma correcao para [X]!")
- [ ] Monitore o dia todo ate meia-noite PT

DEPOIS:
- [ ] Agradeca a todos que apoiaram
- [ ] Escreva um post "licoes aprendidas" (bom para conteudo no Twitter/blog)
- [ ] Incorpore o selo do PH na sua landing page (prova social)
```

> **Erro Comum:** Lancar no Product Hunt antes do produto estar pronto. O PH te da uma chance. Uma vez que voce lanca um produto, nao pode relancar. Espere ate seu produto estar polido, sua landing page converta e seu fluxo de pagamento funcione. PH deve ser seu "grande lancamento" — nao seu lancamento suave.

**Resultados esperados (realistas):**
- Top 5 diario: 3.000-15.000 visitantes, 50-200 upvotes
- Top 10 diario: 1.000-5.000 visitantes, 20-50 upvotes
- Abaixo do top 10: <1.000 visitantes. Impacto duradouro minimo.

### Canal 5: Dev.to / Hashnode / Posts Tecnicos em Blog

**Esforco:** Baixo-medio | **Cronograma:** Resultados de SEO em 1-3 meses | **Natureza:** Cauda longa, compoe para sempre

Escreva posts tecnicos em blog que resolvam problemas relacionados ao seu produto, e mencione seu produto como a solucao.

**Estrategia de conteudo:**

```
Para cada produto, escreva 3-5 posts de blog:

1. "Como [resolver o problema que seu produto resolve] em 2026"
   - Ensine a abordagem manual, depois mencione seu produto como o atalho

2. "Eu construi [produto] em 48 horas — aqui esta o que aprendi"
   - Conteudo build-in-public. Detalhes tecnicos + reflexao honesta.

3. "[Concorrente] vs [Seu Produto]: Comparacao Honesta"
   - Seja genuinamente justo. Mencione onde o concorrente ganha.
   - Isso captura trafego de busca de comparacao de compras.

4. "[Conceito tecnico relacionado ao seu produto] explicado"
   - Educacao pura. Mencione seu produto uma vez no final.

5. "As ferramentas que uso para [dominio do seu produto] em 2026"
   - Formato de lista. Inclua seu produto junto com outros.
```

**Onde publicar:**
- **Dev.to** (https://dev.to) — Grande audiencia de desenvolvedores, bom SEO, gratis
- **Hashnode** (https://hashnode.com) — Bom SEO, opcao de dominio customizado, gratis
- **Seu proprio blog** — Melhor para SEO de longo prazo, voce e dono do conteudo
- **Publique em todos.** Escreva uma vez, publique nas tres plataformas. Use URLs canonicas para evitar penalidades de SEO.

**Resultados esperados por post:**
- Dia 1: 100-1.000 visualizacoes (distribuicao da plataforma)
- Mes 1-3: 50-200 visualizacoes/mes (trafego de busca crescendo)
- Mes 6+: 100-500 visualizacoes/mes (trafego de busca compondo)

Um unico post de blog bem escrito pode gerar 200+ visitantes por mes durante anos. Cinco posts geram 1.000+/mes. Isso compoe.

### Canal 6: Abordagem Direta

**Esforco:** Alto | **Cronograma:** Imediato | **Natureza:** Maior taxa de conversao

Cold email e DMs tem a maior taxa de conversao de qualquer canal — mas tambem o maior esforco por lead. Use isso para produtos de preco mais alto ($99+) ou vendas B2B.

**Template de email para alcancar clientes potenciais:**

```
Assunto: Pergunta rapida sobre [dor especifica deles]

Oi [nome],

Vi seu [tweet/post/comentario] sobre [problema especifico que mencionaram].

Eu construi [nome do produto] especificamente para isso — ele [descricao
de uma frase do que faz].

Voce estaria aberto a testar? Fico feliz em dar acesso gratis
para feedback.

[Seu nome]
[Link para o produto]
```

**Regras para abordagem fria:**
- So entre em contato com pessoas que expressaram publicamente o problema que seu produto resolve
- Referencie o post/comentario especifico deles (prova que voce nao esta mandando emails em massa)
- Ofereca valor (acesso gratis, desconto) ao inves de pedir dinheiro imediatamente
- Mantenha abaixo de 5 frases
- Envie de um endereco de email real (voce@seudominio.com, nao gmail)
- Faca follow-up uma vez apos 3-4 dias. Se nao responder, pare.

**Resultados esperados:**
- Taxa de resposta: 10-20% (cold email para destinatarios relevantes)
- Conversao de resposta para trial: 30-50%
- Conversao de trial para pago: 20-40%
- Conversao efetiva: 1-4% das pessoas contatadas se tornam clientes

Para um produto de $99, enviar email para 100 pessoas = 1-4 vendas = $99-396. Nao escalavel, mas excelente para conseguir primeiros clientes e feedback.

### Canal 7: SEO

**Esforco:** Baixo continuo | **Cronograma:** 3-6 meses para resultados | **Natureza:** Compoe para sempre

SEO e o melhor canal de distribuicao de longo prazo. E lento para comecar mas uma vez que funciona, envia trafego gratis indefinidamente.

**Estrategia de SEO focada em desenvolvedores:**

```
1. Mire palavras-chave long-tail (mais faceis de ranquear):
   Ao inves de: "dashboard components"
   Mire: "tailwind dashboard components react typescript"

2. Crie uma pagina por palavra-chave:
   Cada post de blog ou pagina de docs mira uma consulta de busca especifica

3. Implementacao tecnica:
   - Use geracao de site estatico (Astro, Next.js SSG) para carregamento rapido
   - Adicione meta descriptions em cada pagina
   - Use HTML semantico (hierarquia h1, h2, h3)
   - Adicione alt text em cada imagem
   - Submeta sitemap ao Google Search Console

4. Conteudo que ranqueia para ferramentas de desenvolvedor:
   - Paginas de documentacao (surpreendentemente bom para SEO)
   - Paginas de comparacao ("X vs Y")
   - Paginas de tutorial ("Como fazer X com Y")
   - Paginas de changelog (sinal de conteudo fresco para o Google)
```

```bash
# Submeta seu sitemap ao Google Search Console
# 1. Va a https://search.google.com/search-console
# 2. Adicione sua propriedade (dominio ou prefixo de URL)
# 3. Verifique propriedade (registro DNS TXT ou arquivo HTML)
# 4. Submeta a URL do seu sitemap: seudominio.com/sitemap.xml

# Se usando Astro:
pnpm add @astrojs/sitemap
# Sitemap e gerado automaticamente em /sitemap.xml

# Se usando Next.js, adicione ao next-sitemap.config.js:
# pnpm add next-sitemap
```

**Resultados esperados:**
- Mes 1-3: Trafego organico minimo (<100/mes)
- Mes 3-6: Trafego crescente (100-500/mes)
- Mes 6-12: Trafego significativo (500-5.000/mes)
- Mes 12+: Trafego compondo que cresce sem esforco

{@ temporal market_timing @}

### Framework de Selecao de Canal

Voce nao pode fazer todos os sete bem. Escolha 2-3 baseado nesta matriz:

| Se voce esta... | Priorize | Pule |
|---|---|---|
| Lancando neste fim de semana | Reddit + HN | SEO, Twitter (muito lento) |
| Construindo audiencia primeiro | Twitter + Blog posts | Abordagem direta, PH |
| Vendendo um produto de $99+ | Abordagem direta + HN | Dev.to (audiencia espera gratis) |
| Jogando o jogo longo | SEO + Blog posts + Twitter | PH (uma chance, use depois) |
| Nao falante de ingles | Dev.to + Reddit (global) | HN (centrado nos EUA) |

### Sua Vez

1. **Escolha seus 2-3 canais.** Baseado na matriz acima e no tipo do seu produto, escolha os canais em que voce vai focar. Escreva com seu cronograma planejado para cada um.

2. **Escreva seu post no Reddit.** Usando o template da Licao 1, escreva o rascunho do seu post no r/SideProject agora. Salve. Voce vai postar no dia do lancamento.

3. **Escreva seu primeiro post de blog.** Rascunhe um post "Como [resolver o problema que seu produto resolve]." Isso vai para o Dev.to ou seu blog na primeira semana apos o lancamento. Mire em 1.500-2.000 palavras.

4. **Configure o Google Search Console.** Isso leva 5 minutos e te da dados de SEO desde o dia um. Faca antes de lancar para ter dados de baseline.

---

## Licao 6: Seu Checklist de Lancamento

*"Esperanca nao e uma estrategia de lancamento. Checklists sao."*

### O Checklist Pre-Lancamento

Passe por cada item. Nao lance ate que cada item "Obrigatorio" esteja marcado. Itens "Recomendados" podem ser feitos na Semana 1 se necessario.

**Produto (Obrigatorio):**

```
- [ ] Feature principal funciona como descrito na landing page
- [ ] Sem bugs criticos no fluxo compra → entrega
- [ ] Funciona no Chrome, Firefox e Safari (para produtos web)
- [ ] Landing page responsiva para mobile (50%+ do trafego e mobile)
- [ ] Mensagens de erro sao uteis, nao stack traces
- [ ] Estados de carregamento para qualquer operacao assincrona
```

**Landing Page (Obrigatorio):**

```
- [ ] Titulo claro: o que faz em 8 palavras ou menos
- [ ] Declaracao do problema: 3 dores na linguagem do cliente
- [ ] Secao de solucao: screenshots ou demos do produto
- [ ] Precos: visiveis, claros, com botao de compra
- [ ] Call to action: um botao principal, visivel acima da dobra
- [ ] Politica de privacidade linkada no rodape
- [ ] Termos de servico linkados no rodape
```

**Pagamentos (Obrigatorio):**

```
- [ ] Fluxo de checkout testado ponta a ponta em modo teste
- [ ] Fluxo de checkout testado ponta a ponta em modo real (compra teste de $1)
- [ ] Webhook recebe confirmacao de pagamento
- [ ] Cliente recebe acesso ao produto apos pagamento
- [ ] Processo de reembolso documentado (voce VAI receber pedidos de reembolso)
- [ ] Recibo/nota fiscal enviado automaticamente
```

**Infraestrutura (Obrigatorio):**

```
- [ ] Dominio customizado apontando para o site no ar
- [ ] HTTPS funcionando (cadeado verde)
- [ ] Monitoramento de uptime ativo
- [ ] Script de analytics instalado e recebendo dados
- [ ] Email de contato funcionando (voce@seudominio.com)
```

**Distribuicao (Obrigatorio):**

```
- [ ] Post no Reddit rascunhado e pronto
- [ ] Post Show HN rascunhado e pronto (se aplicavel)
- [ ] Thread de lancamento no Twitter rascunhada
- [ ] 2-3 comunidades identificadas para compartilhamento
```

**Recomendado (Semana 1):**

```
- [ ] Meta tags OpenGraph para previews de compartilhamento social
- [ ] Pagina 404 customizada
- [ ] Pagina ou secao de FAQ
- [ ] Sequencia de email de onboarding do cliente (boas-vindas + primeiros passos)
- [ ] Pagina de changelog (mesmo que vazia — mostra compromisso com atualizacoes)
- [ ] Post de blog: "Eu construi [produto] em 48 horas"
- [ ] Google Search Console verificado e sitemap submetido
```

### Itens de Acao Pos-Lancamento

**Dia 1 (Dia do Lancamento):**

```
Manha:
- [ ] Poste no Reddit (r/SideProject + 1 subreddit de nicho)
- [ ] Poste Show HN (se aplicavel)
- [ ] Poste thread de lancamento no Twitter

O dia todo:
- [ ] Responda a CADA comentario no Reddit, HN e Twitter
- [ ] Monitore logs de erro e analytics em tempo real
- [ ] Corrija qualquer bug descoberto por usuarios imediatamente
- [ ] Envie email de agradecimento pessoal para cada cliente

Noite:
- [ ] Verifique metricas: visitantes, taxa de conversao, receita
- [ ] Tire screenshot do seu painel de analytics (voce vai querer isso depois)
- [ ] Escreva os 3 feedbacks mais comuns
```

**Semana 1:**

```
- [ ] Responda a todos os feedbacks e solicitacoes de suporte em 24 horas
- [ ] Corrija os top 3 bugs/problemas identificados durante o lancamento
- [ ] Escreva e publique seu primeiro post de blog
- [ ] Envie email de follow-up para todos os clientes pedindo feedback
- [ ] Revise analytics: quais paginas tem as maiores taxas de rejeicao?
- [ ] Configure um metodo simples de coleta de feedback (email, Typeform ou Canny)

Metricas semanais para registrar:
| Metrica                | Alvo      | Real   |
|------------------------|-----------|--------|
| Visitantes unicos      | 500+      |        |
| Taxa de clique checkout| 2-5%      |        |
| Conversao de compra    | 1-3%      |        |
| Receita                | $50+      |        |
| Solicitacoes de suporte| <10       |        |
| Pedidos de reembolso   | <2        |        |
```

**Mes 1:**

```
- [ ] Lance 4 melhorias semanais baseadas em feedback de clientes
- [ ] Publique 2+ posts de blog (construindo SEO)
- [ ] Colete 3+ depoimentos de clientes
- [ ] Adicione depoimentos a landing page
- [ ] Avalie precos: muito alto? muito baixo? (revise dados de conversao)
- [ ] Planeje seu "grande lancamento" no Product Hunt (se aplicavel)
- [ ] Comece a construir lista de email para lancamentos futuros de produtos
- [ ] Revise e ajuste sua estrategia de canal de distribuicao

Revisao financeira mensal:
| Categoria                | Valor     |
|--------------------------|-----------|
| Receita bruta            | $         |
| Taxas do processador     | $         |
| Custos de hosting/infra  | $         |
| Custos de API            | $         |
| Lucro liquido            | $         |
| Horas investidas         |           |
| Taxa horaria efetiva     | $         |
```

### O Painel de Metricas

Configure um painel de metricas simples que voce verifica diariamente. Nao precisa ser sofisticado — uma planilha funciona.

```
=== METRICAS DIARIAS (verifique toda manha) ===

Data: ___
Visitantes ontem: ___
Novos clientes ontem: ___
Receita ontem: $___
Solicitacoes de suporte: ___
Uptime: ___%

=== METRICAS SEMANAIS (verifique toda segunda) ===

Semana de: ___
Total de visitantes: ___
Total de clientes: ___
Receita total: $___
Taxa de conversao: ___% (clientes / visitantes)
Pagina mais visitada: ___
Principal fonte de trafego: ___
Principal tema de feedback: ___

=== METRICAS MENSAIS (verifique no 1o do mes) ===

Mes: ___
Receita total: $___
Despesas totais: $___
Lucro liquido: $___
Total de clientes: ___
Reembolsos: ___
Taxa de churn (assinaturas): ___%
MRR (Receita Mensal Recorrente): $___
Taxa de crescimento vs. mes passado: ___%
```

**Configuracao de analytics respeitando privacidade:**

```javascript
// Se usando Plausible, voce obtem a maioria disso no painel deles.
// Para rastreamento de eventos customizados:

// Rastrear cliques no checkout
document.querySelector('#buy-button').addEventListener('click', () => {
  plausible('Checkout Click', {
    props: { tier: 'pro', price: '59' }
  });
});

// Rastrear compras bem-sucedidas (chame do seu handler de sucesso do webhook)
plausible('Purchase', {
  props: { tier: 'pro', revenue: '59' }
});
```

### Quando Dobrar a Aposta, Pivotar ou Encerrar

Apos 30 dias de dados, voce tem sinal suficiente para tomar uma decisao:

**Dobrar a Aposta (continue, invista mais):**

```
Sinais:
- Receita esta crescendo semana a semana (mesmo que devagar)
- Clientes estao fornecendo pedidos de features especificas (querem MAIS)
- Taxa de conversao esta estavel ou melhorando
- Voce esta recebendo trafego organico (pessoas te encontrando sem seus posts)
- Pelo menos um cliente disse "isso me economizou [tempo/dinheiro]"

Acoes:
- Aumente esforcos de distribuicao (adicione um canal)
- Lance a feature mais pedida
- Aumente precos levemente
- Comece a construir uma lista de email para lancamentos futuros
```

**Pivotar (mude o angulo, mantenha o nucleo):**

```
Sinais:
- Visitantes mas sem vendas (pessoas estao interessadas mas nao comprando)
- Vendas de audiencia inesperada (pessoas diferentes das que voce mirou)
- Clientes usam o produto de forma diferente do que voce esperava
- Feedback consistentemente aponta para um problema diferente do que voce esta resolvendo

Acoes:
- Reescreva a landing page para a audiencia/caso de uso real
- Ajuste precos baseado na disposicao de pagar da audiencia real
- Repriorize features em direcao ao que as pessoas realmente usam
- Mantenha o codigo, mude o posicionamento
```

**Encerrar (pare, aprenda, construa outra coisa):**

```
Sinais:
- Sem visitantes apesar de esforcos de distribuicao (problema de demanda)
- Visitantes mas zero cliques no checkout (problema de posicionamento/preco
  que persiste apos ajustes)
- Receita estagnada por 4+ semanas sem tendencia de crescimento
- Voce teme trabalhar nisso (motivacao importa para produtos solo)
- O mercado mudou (concorrente lancou, tecnologia mudou)

Acoes:
- Escreva um post-mortem: o que funcionou, o que nao funcionou, o que voce aprendeu
- Salve o codigo — pecas podem ser uteis no seu proximo produto
- Tire uma semana de folga de construir
- Comece o processo de validacao para uma nova ideia
- Isso nao e fracasso. Sao dados. A maioria dos produtos nao funciona.
  Os desenvolvedores que ganham dinheiro sao os que lancam 5 produtos,
  nao os que passam um ano em um.
```

### O Template de Documento de Lancamento

Este e seu entregavel para o Modulo E. Crie este documento e preencha conforme executa seu lancamento.

```markdown
# Documento de Lancamento: [Nome do Produto]

## Pre-Lancamento

### Resumo da Validacao
- **Volume de busca:** [numeros do Google Trends/Ahrefs]
- **Evidencia de threads:** [links para 5+ threads mostrando demanda]
- **Auditoria de concorrentes:** [3+ concorrentes com pontos fortes/fracos]
- **Evidencia "10 pessoas pagariam":** [como voce validou isso]

### Produto
- **URL:** [URL do produto no ar]
- **Dominio:** [dominio comprado]
- **Hospedagem:** [plataforma]
- **Features principais (v0.1):**
  1. [Feature 1]
  2. [Feature 2]
  3. [Feature 3]

### Precos
- **Preco:** $[valor]
- **Estrutura de tiers:** [Basico/Pro/Equipe ou tier unico]
- **Plataforma de pagamento:** [Lemon Squeezy/Stripe]
- **URL de checkout:** [link]

### Legal
- **Politica de privacidade:** [URL]
- **Termos de servico:** [URL]
- **Entidade empresarial:** [tipo ou "pessoa fisica"]

## Lancamento

### Canais de Distribuicao
| Canal   | URL do Post  | Data Postado | Resultados |
|---------|------------|-------------|---------|
| Reddit  | [link]     | [data]      | [visitantes, upvotes] |
| HN      | [link]     | [data]      | [visitantes, pontos] |
| Twitter | [link]     | [data]      | [impressoes, cliques] |

### Metricas do Dia 1
- Visitantes: ___
- Cliques no checkout: ___
- Compras: ___
- Receita: $___

### Metricas da Semana 1
- Total de visitantes: ___
- Total de compras: ___
- Receita total: $___
- Taxa de conversao: ___%
- Principal feedback: ___

### Metricas do Mes 1
- Receita total: $___
- Despesas totais: $___
- Lucro liquido: $___
- Total de clientes: ___
- Decisao: [ ] Dobrar aposta [ ] Pivotar [ ] Encerrar

## Roadmap Pos-Lancamento
- Semana 2: [melhoria planejada]
- Semana 3: [melhoria planejada]
- Semana 4: [melhoria planejada]
- Mes 2: [feature/expansao planejada]

## Licoes Aprendidas
- O que funcionou: ___
- O que nao funcionou: ___
- O que eu faria diferente: ___
```

### Integracao 4DA

> **Integracao 4DA:** Os sinais acionaveis do 4DA classificam conteudo por urgencia. Um sinal "critico" sobre uma vulnerabilidade em um pacote popular significa: construa a correcao ou ferramenta de migracao AGORA, antes de qualquer outra pessoa. Um sinal de "tendencia ascendente" sobre um novo framework significa: construa o starter kit neste fim de semana enquanto a concorrencia e quase zero. O sprint de 48 horas da Licao 1 funciona melhor quando sua ideia vem de um sinal sensivel ao tempo. Conecte seu feed de inteligencia do 4DA ao seu calendario de sprints — quando uma oportunidade de alta urgencia aparecer, bloqueie o proximo fim de semana e execute. A diferenca entre desenvolvedores que capturam oportunidades e os que perdem nao e talento. E velocidade. O 4DA te da o radar. Este modulo te da a sequencia de lancamento. Juntos, eles transformam sinais em receita.

### Sua Vez

1. **Complete o checklist pre-lancamento.** Passe por cada item. Marque cada um como feito ou agende quando voce vai faze-lo. Nao pule os itens "Obrigatorios".

2. **Crie seu Documento de Lancamento.** Copie o template acima na sua ferramenta de documentos preferida. Preencha tudo que voce sabe agora. Deixe campos em branco para metricas que voce vai preencher durante e apos o lancamento.

3. **Defina sua data de lancamento.** Abra seu calendario. Escolha um sabado especifico nas proximas 2 semanas. Escreva. Conte para alguem — um amigo, um parceiro, um seguidor no Twitter. Responsabilidade torna real.

4. **Defina seus criterios de encerramento.** Antes de lancar, decida: "Se eu tiver menos de [X] vendas apos 30 dias apesar de [Y] esforco de distribuicao, eu vou [pivotar/encerrar]." Escreva isso no seu Documento de Lancamento. Ter criterios pre-estabelecidos previne voce de despejar meses em um produto morto por causa da falacia do custo afundado.
{? if progress.completed("S") ?}
   Consulte de volta seu Documento de Stack Soberano do Modulo S — suas restricoes de orcamento e custos operacionais definem o que "lucrativo" significa para sua situacao especifica.
{? endif ?}

5. **Lance.** Voce tem o playbook. Voce tem as ferramentas. Voce tem o conhecimento. A unica coisa que resta e o ato. A internet esta esperando.

---

## Modulo E: Completo

### O Que Voce Construiu em Duas Semanas

{? if dna.identity_summary ?}
> **Sua identidade de desenvolvedor:** {= dna.identity_summary | fallback("Ainda nao perfilado") =}. Tudo que voce construiu neste modulo alavanca essa identidade — sua velocidade de lancamento e uma funcao da sua expertise existente.
{? endif ?}

Olhe para o que voce agora tem que nao tinha quando comecou este modulo:

1. **Um framework de execucao de 48 horas** que voce pode repetir para cada produto que construir — ideia validada a produto no ar em um fim de semana.
2. **Uma mentalidade de lancamento** que prioriza existencia sobre perfeicao, dados sobre achismos, e iteracao sobre planejamento.
3. **Uma estrategia de precificacao** fundamentada em psicologia real e numeros reais, nao esperanca e subprecificacao.
4. **Uma fundacao legal** que te protege sem te paralisar — politica de privacidade, termos, plano de entidade.
5. **Um playbook de distribuicao** com templates especificos, timing e resultados esperados para sete canais.
6. **Um checklist de lancamento e sistema de acompanhamento** que transforma caos em processo — repetivel, mensuravel, aprimoravel.
7. **Um produto no ar, aceitando pagamentos, com humanos reais visitando.**

Esse ultimo e o que importa. Todo o resto e preparacao. O produto e a prova.

### O Que Vem a Seguir: Modulo E2 — Vantagem em Evolucao

O Modulo E1 te levou ao lancamento. O Modulo E2 te mantem a frente.

Aqui esta o que o Modulo E2 cobre:

- **Sistemas de deteccao de tendencias** — como identificar oportunidades 2-4 semanas antes de se tornarem obvias
- **Monitoramento competitivo** — rastrear o que outros no seu espaco estao construindo e precificando
- **Surfando ondas tecnologicas** — quando adotar nova tecnologia nos seus produtos e quando esperar
- **Desenvolvimento de clientes** — transformando seus primeiros 10 clientes no seu conselho consultivo de produto
- **A decisao do segundo produto** — quando construir o produto #2 vs. melhorar o produto #1

Os desenvolvedores que ganham renda consistente nao sao os que lancam uma vez. Sao os que lancam, iteram e ficam a frente do mercado. O Modulo E2 te da o sistema para ficar a frente.

### O Roadmap Completo STREETS

| Modulo | Titulo | Foco | Duracao |
|--------|--------|------|---------|
| **S** | Setup Soberano | Infraestrutura, legal, orcamento | Semanas 1-2 |
| **T** | Fossos Tecnicos | Vantagens defensaveis, ativos proprietarios | Semanas 3-4 |
| **R** | Motores de Receita | Playbooks especificos de monetizacao com codigo | Semanas 5-8 |
| **E** | Manual de Execucao | Sequencias de lancamento, precos, primeiros clientes | Semanas 9-10 (completo) |
| **E** | Vantagem em Evolucao | Ficar a frente, deteccao de tendencias, adaptacao | Semanas 11-12 |
| **T** | Automacao Tatica | Automatizando operacoes para renda passiva | Semanas 13-14 |
| **S** | Empilhando Fontes | Multiplas fontes de renda, estrategia de portfolio | Semanas 15-16 |

Voce passou da metade. Voce tem um produto no ar. Isso te coloca a frente de 95% dos desenvolvedores que querem construir renda independente mas nunca chegam tao longe.

> **Progresso STREETS:** {= progress.completed_count | fallback("0") =} de {= progress.total_count | fallback("7") =} modulos completados. {? if progress.completed_modules ?}Completados: {= progress.completed_modules | fallback("Nenhum ainda") =}.{? endif ?}

Agora faca crescer.

---

**Seu produto esta no ar. Seu checkout funciona. Humanos podem te pagar dinheiro.**

**Tudo depois disso e otimizacao. E otimizacao e a parte divertida.**

*Seu setup. Suas regras. Sua receita.*
