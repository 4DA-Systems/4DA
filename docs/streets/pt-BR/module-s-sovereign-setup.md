# Modulo S: Configuracao Soberana

**Curso STREETS de Renda para Desenvolvedores — Modulo Gratuito**
*Semanas 1-2 | 6 Licoes | Entregavel: Seu Documento do Stack Soberano*

> "Seu rig e sua infraestrutura de negocios. Configure como tal."

---

Voce ja possui a ferramenta mais poderosa de geracao de renda que a maioria das pessoas nunca tera: uma estacao de trabalho de desenvolvedor com conexao a internet, computacao local e as habilidades para conectar tudo.

A maioria dos desenvolvedores trata seu rig como um produto de consumo. Algo em que jogam, programam, navegam. Mas essa mesma maquina — a que esta embaixo da sua mesa agora — pode executar inferencia, servir APIs, processar dados e gerar receita 24 horas por dia enquanto voce dorme.

Este modulo e sobre olhar para o que voce ja tem atraves de uma lente diferente. Nao "o que posso construir?" mas "o que posso vender?"

Ao final destas duas semanas, voce tera:

- Um inventario claro das suas capacidades de geracao de renda
- Um stack LLM local de nivel producao
- Uma base legal e financeira (mesmo que minima)
- Um Documento do Stack Soberano escrito que se tornara seu plano de negocios

Nada de conversa fiada. Nada de "apenas acredite em voce mesmo." Numeros reais, comandos reais, decisoes reais.

{@ mirror sovereign_readiness @}

Vamos comecar.

---

## Licao 1: A Auditoria do Rig

*"Voce nao precisa de uma 4090. Aqui esta o que realmente importa."*

### Sua Maquina E um Ativo de Negocios

Quando uma empresa avalia sua infraestrutura, ela nao apenas lista especificacoes — ela mapeia capacidades para oportunidades de receita. E exatamente isso que voce vai fazer agora.

{? if computed.profile_completeness != "0" ?}
> **Seu Rig Atual:** {= profile.cpu.model | fallback("CPU desconhecida") =} ({= profile.cpu.cores | fallback("?") =} nucleos / {= profile.cpu.threads | fallback("?") =} threads), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("Sem GPU dedicada") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} livre / {= profile.storage.total | fallback("?") =} total ({= profile.storage.type | fallback("desconhecido") =}), rodando {= profile.os.name | fallback("SO desconhecido") =} {= profile.os.version | fallback("") =}.
{? endif ?}

Abra um terminal e execute os passos a seguir. Anote cada numero. Voce vai precisar deles para o Documento do Stack Soberano na Licao 6.

### Inventario de Hardware

#### CPU

```bash
# Linux/Mac
lscpu | grep "Model name\|CPU(s)\|Thread(s)"
# ou
cat /proc/cpuinfo | grep "model name" | head -1
nproc

# Windows (PowerShell)
Get-CimInstance -ClassName Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors

# macOS
sysctl -n machdep.cpu.brand_string
sysctl -n hw.ncpu
```

**O que importa para a renda:**
- A contagem de nucleos determina quantas tarefas concorrentes seu rig pode lidar. Rodar um LLM local enquanto simultaneamente processa um job em lote requer paralelismo real.
{? if profile.cpu.cores ?}
- *Seu {= profile.cpu.model | fallback("CPU") =} tem {= profile.cpu.cores | fallback("?") =} nucleos — verifique a tabela de requisitos abaixo para ver quais motores de receita seu CPU suporta.*
{? endif ?}
- Para a maioria dos motores de receita neste curso, qualquer CPU moderna 8+ nucleos dos ultimos 5 anos e suficiente.
- Se voce roda LLMs locais apenas em CPU (sem GPU), voce quer 16+ nucleos. Um Ryzen 7 5800X ou Intel i7-12700 e o minimo pratico.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**O que importa para a renda:**
- 16 GB: Minimo absoluto. Voce pode rodar modelos 7B e fazer trabalho basico de automacao.
- 32 GB: Confortavel. Rode modelos 13B localmente, gerencie multiplos projetos, mantenha seu ambiente de desenvolvimento rodando junto com cargas de trabalho de renda.
- 64 GB+: Voce pode rodar modelos 30B+ em CPU, ou manter multiplos modelos carregados. E aqui que as coisas ficam interessantes para vender servicos de inferencia.
{? if profile.ram.total ?}
*Seu sistema tem {= profile.ram.total | fallback("?") =} de RAM. Verifique a tabela acima para ver em qual nivel de capacidade voce esta — isso afeta diretamente quais modelos locais sao praticos para suas cargas de trabalho de renda.*
{? endif ?}

#### GPU

```bash
# NVIDIA
nvidia-smi

# Verificar VRAM especificamente
nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv

# AMD (Linux)
rocm-smi

# macOS (Apple Silicon)
system_profiler SPDisplaysDataType
```

**O que importa para a renda:**

Esta e a especificacao com que as pessoas ficam obcecadas, e aqui esta a verdade honesta: **sua GPU determina seu nivel de LLM local, e seu nivel de LLM local determina quais fluxos de renda rodam mais rapido.** Mas nao determina se voce pode ganhar dinheiro ou nao.

| VRAM | Capacidade LLM | Relevancia para Renda |
|------|----------------|----------------------|
| 0 (apenas CPU) | Modelos 7B a ~5 tokens/seg | Processamento em lote, trabalho assincrono. Lento mas funcional. |
| 6-8 GB (RTX 3060, etc.) | Modelos 7B a ~30 tok/seg, 13B quantizado | Bom o suficiente para a maioria dos fluxos de renda de automacao. |
| 12 GB (RTX 3060 12GB, 4070) | 13B a velocidade total, 30B quantizado | Ponto ideal. A maioria dos motores de receita funciona bem aqui. |
| 16-24 GB (RTX 4090, 3090) | Modelos 30B-70B | Nivel premium. Venda qualidade que outros nao conseguem igualar localmente. |
| 48 GB+ (GPU dupla, A6000) | 70B+ em velocidade | Inferencia local de nivel enterprise. Vantagem competitiva seria. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ usando memoria unificada | Excelente eficiencia. Custo de energia menor que equivalente NVIDIA. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **Sua GPU:** {= profile.gpu.model | fallback("Desconhecida") =} com {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}voce esta no nivel premium. Modelos 30B-70B estao ao alcance localmente. Isso e uma vantagem competitiva seria.{? elif computed.gpu_tier == "sweet_spot" ?}voce esta no ponto ideal. 13B a velocidade total, 30B quantizado. A maioria dos motores de receita funciona bem aqui.{? elif computed.gpu_tier == "capable" ?}voce pode rodar modelos 7B em boa velocidade e 13B quantizado. Bom o suficiente para a maioria dos fluxos de renda de automacao.{? else ?}voce tem aceleracao GPU disponivel. Verifique a tabela acima para ver onde voce se encaixa.{? endif ?}
{? else ?}
> **Nenhuma GPU dedicada detectada.** Voce vai rodar inferencia em CPU, o que significa ~5-12 tokens/seg em modelos 7B. Tudo bem para processamento em lote e trabalho assincrono. Use chamadas de API para preencher a lacuna de velocidade para saida voltada ao cliente.
{? endif ?}

> **Papo Reto:** Se voce tem uma RTX 3060 12GB, voce esta em uma posicao melhor que 95% dos desenvolvedores tentando monetizar IA. Pare de esperar por uma 4090. A 3060 12GB e o Honda Civic da IA local — confiavel, eficiente, da conta do recado. O dinheiro que voce gastaria em um upgrade de GPU e melhor gasto em creditos de API para qualidade voltada ao cliente enquanto seus modelos locais lidam com o trabalho pesado.

#### Armazenamento

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**O que importa para a renda:**
- Modelos LLM ocupam espaco: modelo 7B = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (quantizado).
- Voce precisa de espaco para dados de projeto, bancos de dados, caches e artefatos de saida.
- SSD e inegociavel para qualquer coisa voltada ao cliente. Carregamento de modelo a partir de HDD adiciona 30-60 segundos de tempo de inicializacao.
- Minimo pratico: 500 GB SSD com pelo menos 100 GB livres.
- Confortavel: 1 TB SSD. Mantenha modelos no SSD, arquive no HDD.
{? if profile.storage.free ?}
*Voce tem {= profile.storage.free | fallback("?") =} livre em {= profile.storage.type | fallback("seu drive") =}. {? if profile.storage.type == "SSD" ?}Bom — SSD significa carregamento rapido de modelos.{? elif profile.storage.type == "NVMe" ?}Excelente — NVMe e a opcao mais rapida para carregamento de modelos.{? else ?}Considere um SSD se voce ainda nao tem um — faz uma diferenca real nos tempos de carregamento de modelos.{? endif ?}*
{? endif ?}

#### Rede

```bash
# Teste de velocidade rapido (instale speedtest-cli se necessario)
# pip install speedtest-cli
speedtest-cli --simple

# Ou apenas verifique seu plano
# A velocidade de upload importa mais que download para servir conteudo
```

**O que importa para a renda:**
{? if profile.network.download ?}
*Sua conexao: {= profile.network.download | fallback("?") =} down / {= profile.network.upload | fallback("?") =} up.*
{? endif ?}
- Velocidade de download: 50+ Mbps. Necessaria para baixar modelos, pacotes e dados.
- Velocidade de upload: Este e o gargalo que a maioria das pessoas ignora. Se voce esta servindo qualquer coisa (APIs, resultados processados, entregas), upload importa.
  - 10 Mbps: Adequado para entrega assincrona (arquivos processados, resultados em lote).
  - 50+ Mbps: Necessario se voce esta rodando qualquer tipo de endpoint de API local que servicos externos acessam.
  - 100+ Mbps: Confortavel para tudo neste curso.
- Latencia: Abaixo de 50ms para os principais provedores de nuvem. Execute `ping api.openai.com` e `ping api.anthropic.com` para verificar.

#### Uptime

Esta e a especificacao em que ninguem pensa, mas que separa amadores de pessoas que ganham dinheiro enquanto dormem.

Pergunte-se:
- Seu rig pode rodar 24/7? (Energia, refrigeracao, ruido)
- Voce tem um nobreak para quedas de energia?
- Sua conexao de internet e estavel o suficiente para workflows automatizados?
- Voce pode acessar sua maquina remotamente via SSH se algo quebrar?

Se voce nao pode rodar 24/7, tudo bem — muitos fluxos de renda neste curso sao jobs em lote assincronos que voce dispara manualmente. Mas os que geram renda verdadeiramente passiva requerem uptime.

{? if computed.os_family == "windows" ?}
**Setup rapido de uptime (Windows):** Use o Agendador de Tarefas para reinicio automatico, ative a Area de Trabalho Remota ou instale o Tailscale para acesso remoto, e configure seu BIOS para "restaurar na perda de energia CA" para recuperar de quedas de energia.
{? endif ?}

**Setup rapido de uptime (se voce quiser):**

```bash
# Ative Wake-on-LAN (verifique o BIOS)
# Configure acesso SSH
sudo systemctl enable ssh  # Linux

# Auto-reinicio em crash (exemplo de servico systemd)
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

### A Matematica da Eletricidade

As pessoas ou ignoram isso ou catastrofizam. Vamos fazer matematica real.

**Medindo seu consumo real de energia:**

```bash
# Se voce tem um medidor Kill-A-Watt ou tomada inteligente com monitoramento:
# Meça em repouso, sob carga (rodando inferencia) e no maximo (GPU em utilizacao total)

# Estimativas aproximadas se voce nao tem um medidor:
# Desktop (sem GPU, em repouso): 60-100W
# Desktop (GPU de faixa media, em repouso): 80-130W
# Desktop (GPU de ponta, em repouso): 100-180W
# Desktop (GPU sob carga de inferencia): adicione 50-80% do TDP da GPU
# Laptop: 15-45W
# Mac Mini M2: 7-15W (serio)
# Laptop Apple Silicon: 10-30W
```

**Calculo de custo mensal:**

```
Custo mensal = (Watts / 1000) x Horas x Preco por kWh

Exemplo: Desktop com RTX 3060, rodando inferencia 8 horas/dia, em repouso 16 horas/dia
- Inferencia: (250W / 1000) x 8h x 30 dias x $0.12/kWh = $7.20/mes
- Repouso: (100W / 1000) x 16h x 30 dias x $0.12/kWh = $5.76/mes
- Total: ~$13/mes

Exemplo: Mesmo rig, inferencia 24/7
- (250W / 1000) x 24h x 30 dias x $0.12/kWh = $21.60/mes

Exemplo: Mac Mini M2, 24/7
- (12W / 1000) x 24h x 30 dias x $0.12/kWh = $1.04/mes
```

{? if regional.country ?}
Sua tarifa de eletricidade: aproximadamente {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh (baseado nas medias de {= regional.country | fallback("sua regiao") =}). Verifique sua conta de luz real — as tarifas variam por fornecedor e horario.
{? else ?}
A media de eletricidade nos EUA e cerca de $0.12/kWh. Verifique sua tarifa real — varia muito. California pode ser $0.25/kWh. Alguns paises europeus chegam a $0.35/kWh. Partes do Centro-Oeste dos EUA sao $0.08/kWh.
{? endif ?}

**O ponto:** Rodar seu rig 24/7 para gerar renda custa algo entre {= regional.currency_symbol | fallback("$") =}1-{= regional.currency_symbol | fallback("$") =}30/mes em eletricidade. Se seus fluxos de renda nao cobrem isso, o problema nao e a eletricidade — e o fluxo de renda.

### Especificacoes Minimas por Tipo de Motor de Receita

Aqui esta uma previa de para onde estamos indo no curso STREETS completo. Por enquanto, apenas verifique onde seu rig se encaixa:

| Motor de Receita | CPU | RAM | GPU | Armazenamento | Rede |
|------------------|-----|-----|-----|---------------|------|
| **Automacao de conteudo** (posts de blog, newsletters) | 4+ nucleos | 16 GB | Opcional (fallback API) | 50 GB livres | 10 Mbps up |
| **Servicos de processamento de dados** | 8+ nucleos | 32 GB | Opcional | 200 GB livres | 50 Mbps up |
| **Servicos de API de IA local** | 8+ nucleos | 32 GB | 8+ GB VRAM | 100 GB livres | 50 Mbps up |
| **Ferramentas de geracao de codigo** | 8+ nucleos | 16 GB | 8+ GB VRAM ou API | 50 GB livres | 10 Mbps up |
| **Processamento de documentos** | 4+ nucleos | 16 GB | Opcional | 100 GB livres | 10 Mbps up |
| **Agentes autonomos** | 8+ nucleos | 32 GB | 12+ GB VRAM | 100 GB livres | 50 Mbps up |

> **Erro Comum:** "Preciso atualizar meu hardware antes de poder comecar." Nao. Comece com o que voce tem. Use chamadas de API para preencher as lacunas que seu hardware nao cobre. Atualize quando a receita justificar — nao antes.

{@ insight engine_ranking @}

### Checkpoint Licao 1

Voce agora deve ter anotado:
- [ ] Modelo de CPU, nucleos e threads
- [ ] Quantidade de RAM
- [ ] Modelo de GPU e VRAM (ou "nenhuma")
- [ ] Armazenamento disponivel
- [ ] Velocidades de rede (down/up)
- [ ] Custo mensal estimado de eletricidade para operacao 24/7
- [ ] Para quais categorias de motores de receita seu rig se qualifica

Guarde esses numeros. Voce vai inseri-los no seu Documento do Stack Soberano na Licao 6.

{? if computed.profile_completeness != "0" ?}
> **O 4DA ja coletou a maioria desses numeros para voce.** Verifique os resumos personalizados acima — seu inventario de hardware esta parcialmente pre-preenchido pela deteccao do sistema.
{? endif ?}

*No curso STREETS completo, o Modulo R (Motores de Receita) te da playbooks especificos, passo a passo, para cada tipo de motor listado acima — incluindo o codigo exato para construi-los e implanta-los.*

---

## Licao 2: O Stack LLM Local

*"Configure o Ollama para uso em producao — nao apenas para bate-papo."*

### Por Que LLMs Locais Importam para a Renda

Toda vez que voce chama a API da OpenAI, voce esta pagando aluguel. Toda vez que voce roda um modelo localmente, essa inferencia e gratuita apos a configuracao inicial. A matematica e simples:

- GPT-4o: ~$5 por milhao de tokens de entrada, ~$15 por milhao de tokens de saida
- Claude 3.5 Sonnet: ~$3 por milhao de tokens de entrada, ~$15 por milhao de tokens de saida
- Llama 3.1 8B local: $0 por milhao de tokens (apenas eletricidade)

Se voce esta construindo servicos que processam milhares de requisicoes, a diferenca entre $0 e $5-$15 por milhao de tokens e a diferenca entre lucro e empate.

Mas aqui esta a nuance que a maioria das pessoas nao percebe: **modelos locais e de API servem papeis diferentes em um stack de renda.** Modelos locais lidam com volume. Modelos de API lidam com saida de qualidade critica voltada ao cliente. Seu stack precisa de ambos.

### Instalando o Ollama

{? if settings.has_llm ?}
> **Voce ja tem um LLM configurado:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("modelo desconhecido") =}. Se o Ollama ja esta rodando, pule para o "Guia de Selecao de Modelos" abaixo.
{? endif ?}

Ollama e a fundacao. Transforma sua maquina em um servidor de inferencia local com uma API limpa.

```bash
# Linux
curl -fsSL https://ollama.com/install.sh | sh

# macOS
# Baixe de https://ollama.com ou:
brew install ollama

# Windows
# Baixe o instalador de https://ollama.com
# Ou use winget:
winget install Ollama.Ollama
```

{? if computed.os_family == "windows" ?}
> **Windows:** Use o instalador de ollama.com ou `winget install Ollama.Ollama`. O Ollama roda como servico em segundo plano automaticamente apos a instalacao.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` e o caminho mais rapido. O Ollama aproveita a memoria unificada do Apple Silicon — seus {= profile.ram.total | fallback("sistema") =} de RAM sao compartilhados entre cargas de trabalho de CPU e GPU.
{? elif computed.os_family == "linux" ?}
> **Linux:** O script de instalacao cuida de tudo. Se voce esta rodando {= profile.os.name | fallback("Linux") =}, o Ollama se instala como servico systemd.
{? endif ?}

Verifique a instalacao:

```bash
ollama --version
# Deve mostrar versao 0.5.x ou superior (verifique https://ollama.com/download para a versao mais recente)

# Inicie o servidor (se nao iniciou automaticamente)
ollama serve

# Em outro terminal, teste:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **Nota sobre versao:** O Ollama lanca atualizacoes frequentemente. Os comandos de modelo e flags neste modulo foram verificados com Ollama v0.5.x (inicio de 2026). Se voce esta lendo isso mais tarde, verifique [ollama.com/download](https://ollama.com/download) para a versao mais recente e [ollama.com/library](https://ollama.com/library) para nomes de modelos atuais. Os conceitos fundamentais nao mudam, mas tags especificas de modelos (ex., `llama3.1:8b`) podem ser substituidas por versoes mais recentes.

### Guia de Selecao de Modelos

Nao baixe todo modelo que voce ve. Seja estrategico. Aqui esta o que baixar e quando usar cada um.

{? if computed.llm_tier ?}
> **Seu nivel de LLM (baseado no hardware):** {= computed.llm_tier | fallback("desconhecido") =}. As recomendacoes abaixo sao marcadas para que voce possa focar no nivel que corresponde ao seu rig.
{? endif ?}

#### Nivel 1: O Cavalo de Batalha (modelos 7B-8B)

```bash
# Baixe seu modelo cavalo de batalha
ollama pull llama3.1:8b
# Alternativa: mistral (bom para linguas europeias)
ollama pull mistral:7b
```

**Use para:**
- Classificacao de texto ("Este email e spam ou legitimo?")
- Resumo (condensar documentos longos em pontos-chave)
- Extracao simples de dados (extrair nomes, datas, valores do texto)
- Analise de sentimento
- Tagging e categorizacao de conteudo
- Geracao de embeddings (se usar um modelo com suporte a embedding)

**Performance (tipica):**
- RTX 3060 12GB: ~40-60 tokens/segundo
- RTX 4090: ~100-130 tokens/segundo
- M2 Pro 16GB: ~30-45 tokens/segundo
- Apenas CPU (Ryzen 7 5800X): ~8-12 tokens/segundo

**Comparacao de custos:**
- 1 milhao de tokens via GPT-4o-mini: ~$0.60
- 1 milhao de tokens localmente (modelo 8B): ~$0.003 em eletricidade
- Ponto de equilibrio: ~5.000 tokens (voce economiza desde literalmente a primeira requisicao)

#### Nivel 2: A Escolha Equilibrada (modelos 13B-14B)

```bash
# Baixe seu modelo equilibrado
ollama pull llama3.1:14b
# Ou para tarefas de codigo:
ollama pull deepseek-coder-v2:16b
```

**Use para:**
- Redacao de conteudo (posts de blog, documentacao, copy de marketing)
- Geracao de codigo (funcoes, scripts, boilerplate)
- Transformacao complexa de dados
- Tarefas de raciocinio multi-etapas
- Traducao com nuance

**Performance (tipica):**
- RTX 3060 12GB: ~20-30 tokens/segundo (quantizado)
- RTX 4090: ~60-80 tokens/segundo
- M2 Pro 32GB: ~20-30 tokens/segundo
- Apenas CPU: ~3-6 tokens/segundo (nao pratico para tempo real)

**Quando usar em vez do 7B:** Quando a qualidade da saida do 7B nao e boa o suficiente mas voce nao precisa pagar por chamadas de API. Teste ambos no seu caso de uso real — as vezes o 7B e suficiente e voce esta apenas desperdicando computacao.

{? if computed.gpu_tier == "capable" ?}
> **Territorio do Nivel 3** — Sua {= profile.gpu.model | fallback("GPU") =} consegue lidar com 30B quantizado com algum esforco, mas 70B esta fora de alcance localmente. Considere chamadas de API para tarefas que precisam de qualidade nivel 70B.
{? endif ?}

#### Nivel 3: O Nivel Qualidade (modelos 30B-70B)

```bash
# Baixe estes apenas se voce tem a VRAM necessaria
# 30B precisa de ~20GB VRAM, 70B precisa de ~40GB VRAM (quantizado)
ollama pull llama3.1:70b-instruct-q4_K_M
# Ou o menor mas excelente:
ollama pull qwen2.5:32b
```

**Use para:**
- Conteudo voltado ao cliente que precisa ser excelente
- Analise e raciocinio complexos
- Geracao de conteudo longo
- Tarefas onde a qualidade impacta diretamente se alguem te paga

**Performance (tipica):**
- RTX 4090 (24GB): 70B a ~8-15 tokens/segundo (usavel mas lento)
- GPU dupla ou 48GB+: 70B a ~20-30 tokens/segundo
- M3 Max 64GB: 70B a ~10-15 tokens/segundo

> **Papo Reto:** Se voce nao tem 24GB+ de VRAM, pule os modelos 70B inteiramente. Use chamadas de API para saida de qualidade critica. Um modelo 70B rodando a 3 tokens/segundo da RAM do sistema e tecnicamente possivel mas praticamente inutil para qualquer workflow que gere renda. Seu tempo tem valor.

#### Nivel 4: Modelos de API (Quando o Local Nao E Suficiente)

Modelos locais sao para volume e privacidade. Modelos de API sao para tetos de qualidade e capacidades especializadas.

**Quando usar modelos de API:**
- Saida voltada ao cliente onde qualidade = receita (copy de vendas, conteudo premium)
- Cadeias de raciocinio complexas em que modelos menores falham
- Tarefas de visao/multimodais (analisar imagens, screenshots, documentos)
- Quando voce precisa de saida JSON estruturada com alta confiabilidade
- Quando velocidade importa e seu hardware local e lento

**Tabela de comparacao de custos (inicio de 2025 — verifique precos atuais):**

| Modelo | Entrada (por 1M tokens) | Saida (por 1M tokens) | Melhor Para |
|--------|------------------------|----------------------|-------------|
| GPT-4o-mini | $0.15 | $0.60 | Trabalho de volume barato (quando local nao esta disponivel) |
| GPT-4o | $2.50 | $10.00 | Visao, raciocinio complexo |
| Claude 3.5 Sonnet | $3.00 | $15.00 | Codigo, analise, contexto longo |
| Claude 3.5 Haiku | $0.80 | $4.00 | Rapido, barato, bom equilibrio de qualidade |
| DeepSeek V3 | $0.27 | $1.10 | Economico, performance forte |

**A estrategia hibrida:**
1. LLM local 7B/13B lida com 80% das requisicoes (classificacao, extracao, resumo)
2. API lida com 20% das requisicoes (passo final de qualidade, tarefas complexas, saida voltada ao cliente)
3. Seu custo efetivo: ~$0.50-2.00 por milhao de tokens misturado (em vez de $5-15 apenas API)

Esta abordagem hibrida e como voce constroi servicos com margens saudaveis. Mais sobre isso no Modulo R.

### Configuracao de Producao

Rodar Ollama para trabalho de renda e diferente de rodar para chat pessoal. Aqui esta como configurar corretamente.

{? if computed.has_nvidia ?}
> **GPU NVIDIA detectada ({= profile.gpu.model | fallback("desconhecida") =}).** O Ollama usara automaticamente aceleracao CUDA. Certifique-se de que seus drivers NVIDIA estao atualizados — rode `nvidia-smi` para verificar. Para melhor performance com {= profile.gpu.vram | fallback("sua") =} VRAM, a configuracao `OLLAMA_MAX_LOADED_MODELS` abaixo deve corresponder a quantos modelos cabem na sua VRAM simultaneamente.
{? endif ?}

#### Configure Variaveis de Ambiente

```bash
# Crie/edite a configuracao do Ollama
# Linux: /etc/systemd/system/ollama.service ou variaveis de ambiente
# macOS: ambiente launchctl ou ~/.zshrc
# Windows: Variaveis de Ambiente do Sistema

# Configuracoes principais:
export OLLAMA_HOST=127.0.0.1:11434    # Bind apenas em localhost (seguranca)
export OLLAMA_NUM_PARALLEL=4            # Tratamento de requisicoes concorrentes
export OLLAMA_MAX_LOADED_MODELS=2       # Manter 2 modelos em memoria
export OLLAMA_KEEP_ALIVE=30m            # Manter modelo carregado por 30 min apos ultima requisicao
export OLLAMA_MAX_QUEUE=100             # Enfileirar ate 100 requisicoes
```

#### Crie um Modelfile para Sua Carga de Trabalho

Em vez de usar configuracoes padrao do modelo, crie um Modelfile personalizado ajustado para sua carga de trabalho de renda:

```dockerfile
# Salve como: Modelfile-worker
FROM llama3.1:8b

# Ajustado para saida de producao consistente
PARAMETER temperature 0.3
PARAMETER top_p 0.9
PARAMETER num_ctx 4096
PARAMETER repeat_penalty 1.1

# Prompt de sistema para sua carga de trabalho mais comum
SYSTEM """You are a precise data processing assistant. You follow instructions exactly. You output only what is requested, with no preamble or explanation unless asked. When given structured output formats (JSON, CSV, etc.), you output only the structure with no markdown formatting."""
```

```bash
# Crie seu modelo personalizado
ollama create worker -f Modelfile-worker

# Teste
ollama run worker "Extract all email addresses from this text: Contact us at hello@example.com or support@test.org for more info."
```

#### Batching e Gerenciamento de Fila

Para cargas de trabalho de renda, voce frequentemente precisara processar muitos itens. Aqui esta uma configuracao basica de batching:

```python
#!/usr/bin/env python3
"""
batch_processor.py — Processa itens atraves de LLM local com enfileiramento.
Batching de nivel producao para cargas de trabalho de renda.
"""

import requests
import json
import time
import concurrent.futures
from pathlib import Path

OLLAMA_URL = "http://127.0.0.1:11434/api/generate"
MODEL = "worker"  # Seu modelo personalizado acima
MAX_CONCURRENT = 4
MAX_RETRIES = 3

def process_item(item: dict) -> dict:
    """Processa um unico item atraves do LLM local."""
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
            time.sleep(2 ** attempt)  # Backoff exponencial

def process_batch(items: list[dict], output_file: str = "results.jsonl"):
    """Processa um lote de itens com execucao concorrente."""
    results = []
    start_time = time.time()

    with concurrent.futures.ThreadPoolExecutor(max_workers=MAX_CONCURRENT) as executor:
        future_to_item = {executor.submit(process_item, item): item for item in items}

        for i, future in enumerate(concurrent.futures.as_completed(future_to_item)):
            result = future.result()
            results.append(result)

            # Escrita incremental (nao perca progresso em caso de crash)
            with open(output_file, "a") as f:
                f.write(json.dumps(result) + "\n")

            # Relatorio de progresso
            elapsed = time.time() - start_time
            rate = (i + 1) / elapsed
            remaining = (len(items) - i - 1) / rate if rate > 0 else 0
            print(f"[{i+1}/{len(items)}] {result['status']} | "
                  f"{rate:.1f} itens/seg | "
                  f"ETA: {remaining:.0f}s")

    # Resumo
    succeeded = sum(1 for r in results if r["status"] == "success")
    failed = sum(1 for r in results if r["status"] == "failed")
    total_time = time.time() - start_time

    print(f"\nLote completo: {succeeded} sucesso, {failed} falha, "
          f"{total_time:.1f}s total")

    return results

# Exemplo de uso:
if __name__ == "__main__":
    # Seus itens para processar
    items = [
        {"id": i, "prompt": f"Summarize this in one sentence: {text}"}
        for i, text in enumerate(load_your_data())  # Substitua pela sua fonte de dados
    ]

    results = process_batch(items)
```

### Benchmarking do SEU Rig

Nao confie nos benchmarks de outros. Meça os seus:

```bash
# Script de benchmark rapido
# Salve como: benchmark.sh

#!/bin/bash
MODELS=("llama3.1:8b" "mistral:7b")
PROMPT="Write a detailed 200-word product description for a wireless mechanical keyboard designed for programmers."

for model in "${MODELS[@]}"; do
    echo "=== Benchmarking: $model ==="

    # Aquecimento (primeira execucao carrega o modelo na memoria)
    ollama run "$model" "Hello" > /dev/null 2>&1

    # Execucao cronometrada
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
    echo "Tokens gerados: $TOKENS"
    if [ "$TOKENS" != "N/A" ] && [ "$DURATION" -gt 0 ]; then
        TPS=$(python3 -c "print(f'{$TOKENS / ($DURATION / 1000):.1f}')")
        echo "Velocidade: $TPS tokens/segundo"
    fi
    echo ""
done
```

```bash
chmod +x benchmark.sh
./benchmark.sh
```

Anote seus tokens/segundo para cada modelo. Esse numero determina quais workflows de renda sao praticos para seu rig.

{@ insight stack_fit @}

**Requisitos de velocidade por caso de uso:**
- Processamento em lote (assincrono): 5+ tokens/seg e suficiente (voce nao se importa com latencia)
- Ferramentas interativas (usuario espera): 20+ tokens/seg minimo
- API em tempo real (voltada ao cliente): 30+ tokens/seg para boa UX
- Chat em streaming: 15+ tokens/seg parece responsivo

### Protegendo Seu Servidor de Inferencia Local

{? if computed.os_family == "windows" ?}
> **Nota Windows:** O Ollama no Windows se liga ao localhost por padrao. Verifique com `netstat -an | findstr 11434` no PowerShell. Use o Windows Firewall para bloquear acesso externo a porta 11434.
{? elif computed.os_family == "macos" ?}
> **Nota macOS:** O Ollama no macOS se liga ao localhost por padrao. Verifique com `lsof -i :11434`. O firewall do macOS deve bloquear conexoes externas automaticamente.
{? endif ?}

Sua instancia do Ollama nunca deve ser acessivel pela internet a menos que voce pretenda explicitamente.

```bash
# Verifique que o Ollama esta escutando apenas no localhost
ss -tlnp | grep 11434
# Deve mostrar 127.0.0.1:11434, NAO 0.0.0.0:11434

# Se voce precisa de acesso remoto (ex., de outra maquina na sua LAN):
# Use tunneling SSH em vez de expor a porta
ssh -L 11434:localhost:11434 your-rig-ip

# Regras de firewall (Linux)
sudo ufw deny in 11434
sudo ufw allow from 192.168.1.0/24 to any port 11434  # Apenas LAN, se necessario
```

> **Erro Comum:** Ligar o Ollama em 0.0.0.0 por "conveniencia" e esquecer. Qualquer pessoa que encontre seu IP pode usar sua GPU para inferencia gratuita. Pior, podem extrair pesos do modelo e prompts do sistema. Sempre localhost. Sempre tunnel.

### Checkpoint Licao 2

Voce agora deve ter:
- [ ] Ollama instalado e rodando
- [ ] Pelo menos um modelo cavalo de batalha baixado (llama3.1:8b ou equivalente)
- [ ] Um Modelfile personalizado para sua carga de trabalho esperada
- [ ] Numeros de benchmark: tokens/segundo para cada modelo no seu rig
- [ ] Ollama ligado apenas ao localhost

*No curso STREETS completo, o Modulo T (Fossas Tecnicas) mostra como construir configuracoes de modelos proprietarias, pipelines de fine-tuning e toolchains personalizados que concorrentes nao conseguem facilmente replicar. O Modulo R (Motores de Receita) te da os servicos exatos para construir em cima deste stack.*

---

## Licao 3: A Vantagem da Privacidade

*"Sua configuracao privada E uma vantagem competitiva — nao apenas uma preferencia."*

### Privacidade E um Recurso do Produto, Nao uma Limitacao

A maioria dos desenvolvedores configura infraestrutura local porque valoriza pessoalmente a privacidade, ou porque gosta de mexer. Tudo bem. Mas voce esta deixando dinheiro na mesa se nao percebe que **a privacidade e um dos recursos mais comercializaveis na tecnologia agora.**

Aqui esta o motivo: toda vez que uma empresa envia dados para a API da OpenAI, esses dados passam por terceiros. Para muitos negocios — especialmente na saude, financas, juridico, governo e empresas com sede na UE — isso e um problema real. Nao teorico. Um problema do tipo "nao podemos usar essa ferramenta porque a compliance disse nao".

Voce, rodando modelos localmente na sua maquina, nao tem esse problema.

### O Vento Regulatorio a Favor

O ambiente regulatorio esta se movendo na sua direcao. Rapido.

{? if regional.country == "US" ?}
> **Baseado nos EUA:** As regulamentacoes abaixo que mais importam para voce sao HIPAA, SOC 2, ITAR e leis de privacidade estaduais (California CCPA, etc.). Regulamentacoes da UE ainda importam — afetam sua capacidade de atender clientes europeus, que e um mercado lucrativo.
{? elif regional.country == "GB" ?}
> **Baseado no Reino Unido:** Pos-Brexit, o Reino Unido tem seu proprio framework de protecao de dados (UK GDPR + Data Protection Act 2018). Sua vantagem de processamento local e especialmente forte para atender servicos financeiros do UK e trabalho adjacente ao NHS.
{? elif regional.country == "DE" ?}
> **Baseado na Alemanha:** Voce esta em um dos ambientes de protecao de dados mais rigorosos do mundo. Isso e uma *vantagem* — clientes alemaes ja entendem por que o processamento local importa, e vao pagar por isso.
{? elif regional.country == "AU" ?}
> **Baseado na Australia:** O Privacy Act 1988 e os Australian Privacy Principles (APPs) governam suas obrigacoes. Processamento local e um forte argumento de venda para clientes governamentais e de saude sob o My Health Records Act.
{? endif ?}

**EU AI Act (aplicado de 2024-2026):**
- Sistemas de IA de alto risco precisam de pipelines de processamento de dados documentadas
- Empresas devem demonstrar para onde os dados fluem e quem os processa
- Processamento local simplifica dramaticamente a conformidade
- Empresas da UE estao ativamente procurando provedores de servicos de IA que possam garantir residencia de dados na UE

**GDPR (ja aplicado):**
- "Processamento de dados" inclui enviar texto para uma API de LLM
- Empresas precisam de Acordos de Processamento de Dados com cada terceiro
- Processamento local elimina o terceiro inteiramente
- Este e um verdadeiro argumento de venda: "Seus dados nunca saem da sua infraestrutura. Nao ha DPA de terceiros para negociar."

**Regulamentacoes especificas de setor:**
- **HIPAA (Saude EUA):** Dados de pacientes nao podem ser enviados para APIs de IA ao consumidor sem um BAA (Business Associate Agreement). A maioria dos provedores de IA nao oferece BAAs para acesso a API. Processamento local contorna isso inteiramente.
- **SOC 2 (Enterprise):** Empresas passando por auditorias SOC 2 precisam documentar cada processador de dados. Menos processadores = auditorias mais faceis.
- **ITAR (Defesa EUA):** Dados tecnicos controlados nao podem sair da jurisdicao dos EUA. Provedores de IA em nuvem com infraestrutura internacional sao problematicos.
- **PCI DSS (Financas):** Processamento de dados de cartao tem requisitos rigorosos sobre por onde os dados trafegam.

### Como Posicionar a Privacidade em Conversas de Vendas

Voce nao precisa ser um especialista em conformidade. Voce precisa entender tres frases e saber quando usa-las:

**Frase 1: "Seus dados nunca saem da sua infraestrutura."**
Use quando: Falando com qualquer prospect preocupado com privacidade. Este e o gancho universal.

**Frase 2: "Nenhum acordo de processamento de dados com terceiros necessario."**
Use quando: Falando com empresas europeias ou qualquer empresa com equipe juridica/compliance. Isso economiza semanas de revisao legal para eles.

**Frase 3: "Trilha de auditoria completa, processamento single-tenant."**
Use quando: Falando com enterprise ou industrias regulamentadas. Eles precisam provar seu pipeline de IA para auditores.

**Exemplo de posicionamento (para sua pagina de servicos ou propostas):**

> "Ao contrario dos servicos de IA baseados em nuvem, [Seu Servico] processa todos os dados localmente em hardware dedicado. Seus documentos, codigo e dados nunca saem do ambiente de processamento. Nao ha APIs de terceiros no pipeline, nenhum acordo de compartilhamento de dados para negociar, e log de auditoria completo de cada operacao. Isso torna [Seu Servico] adequado para organizacoes com requisitos rigorosos de manipulacao de dados, incluindo ambientes de conformidade GDPR, HIPAA e SOC 2."

Esse paragrafo, em uma landing page, vai atrair exatamente os clientes que pagam tarifas premium.

### A Justificativa de Preco Premium

Aqui esta o caso de negocios em numeros concretos:

**Servico padrao de processamento de IA (usando APIs de nuvem):**
- Os dados do cliente vao para OpenAI/Anthropic/Google
- Voce esta competindo com todo desenvolvedor que pode chamar uma API
- Taxa de mercado: $0.01-0.05 por documento processado
- Voce esta essencialmente revendendo acesso a API com markup

**Servico de processamento de IA privacy-first (seu stack local):**
- Os dados do cliente ficam na sua maquina
- Voce esta competindo com um pool muito menor de provedores
- Taxa de mercado: $0.10-0.50 por documento processado (premium de 5-10x)
- Voce esta vendendo infraestrutura + expertise + conformidade

O premium da privacidade e real: **de 5x a 10x** sobre servicos de nuvem commodity para a mesma tarefa subjacente. E os clientes que pagam sao mais leais, menos sensiveis a preco e tem orcamentos maiores.

{@ insight competitive_position @}

### Configurando Workspaces Isolados

Se voce tem um emprego fixo (a maioria de voces tem), voce precisa de separacao limpa entre trabalho do empregador e trabalho de renda. Isso nao e apenas protecao legal — e higiene operacional.

{? if computed.os_family == "windows" ?}
> **Dica Windows:** Crie uma conta de usuario Windows separada para trabalho de renda (Configuracoes > Contas > Familia e outros usuarios > Adicionar outra pessoa). Isso te da um ambiente completamente isolado — perfis de navegador separados, caminhos de arquivo separados, variaveis de ambiente separadas. Alterne entre contas com Win+L.
{? endif ?}

**Opcao 1: Contas de usuario separadas (recomendado)**

```bash
# Linux: Crie um usuario dedicado para trabalho de renda
sudo useradd -m -s /bin/bash income
sudo passwd income

# Mude para o usuario income para todo trabalho de renda
su - income

# Todos os projetos de renda, chaves de API e dados ficam em /home/income/
```

**Opcao 2: Workspaces containerizados**

```bash
# Isolamento baseado em Docker
# Crie um container de workspace dedicado

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
    # A VPN, ferramentas, etc. do seu empregador NAO estao neste container
```

**Opcao 3: Maquina fisica separada (mais blindada)**

Se voce esta falando serio e sua renda justifica, uma maquina dedicada elimina todas as perguntas. Um Dell OptiPlex usado com uma RTX 3060 custa $400-600 e se paga no primeiro mes de trabalho para clientes.

**Checklist de separacao minima:**
- [ ] Projetos de renda em um diretorio separado (nunca misturados com repos do empregador)
- [ ] Chaves de API separadas para trabalho de renda (nunca use chaves fornecidas pelo empregador)
- [ ] Perfil de navegador separado para contas relacionadas a renda
- [ ] Trabalho de renda nunca feito em hardware do empregador
- [ ] Trabalho de renda nunca feito na rede do empregador (use sua internet pessoal ou uma VPN)
- [ ] Conta separada no GitHub/GitLab para projetos de renda (opcional mas limpo)

> **Erro Comum:** Usar a chave de API da OpenAI do seu empregador "so para testar" seu projeto pessoal. Isso cria um rastro que o dashboard de faturamento do seu empregador pode ver, e confunde as aguas de PI. Pegue suas proprias chaves. Sao baratas.

### Checkpoint Licao 3

Voce agora deve entender:
- [ ] Por que privacidade e um recurso comercializavel do produto, nao apenas uma preferencia pessoal
- [ ] Quais regulamentacoes criam demanda para processamento de IA local
- [ ] Tres frases para usar em conversas de vendas sobre privacidade
- [ ] Como servicos privacy-first comandam precos premium de 5-10x
- [ ] Como separar trabalho de renda do trabalho para o empregador

*No curso STREETS completo, o Modulo E (Vantagem em Evolucao) ensina como rastrear mudancas regulatorias e se posicionar a frente dos novos requisitos de conformidade antes que seus concorrentes saibam que existem.*

---

## Licao 4: O Minimo Legal

*"Quinze minutos de preparacao legal agora previnem meses de problemas depois."*

### Isto Nao E Aconselhamento Juridico

Sou desenvolvedor, nao advogado. O que segue e uma checklist pratica que a maioria dos desenvolvedores na maioria das situacoes deve abordar. Se sua situacao e complexa (equity no empregador, nao-concorrencia com termos especificos, etc.), gaste $200 em uma consulta de 30 minutos com um advogado trabalhista. E o melhor ROI que voce tera.

### Passo 1: Leia Seu Contrato de Trabalho

Encontre seu contrato de trabalho ou carta de oferta. Procure estas secoes:

**Clausula de cessao de propriedade intelectual** — Procure linguagem como:
- "Todas as invencoes, desenvolvimentos e produtos do trabalho..."
- "...criados durante o periodo de emprego..."
- "...relacionados ao negocio da Empresa ou negocio previsto..."

**Frases-chave que te restringem:**
- "Todo produto do trabalho criado durante o emprego pertence a Empresa" (amplo — potencialmente problematico)
- "Produto do trabalho criado usando recursos da Empresa" (mais restrito — geralmente ok se voce usa seu proprio equipamento)
- "Relacionado ao negocio atual ou previsto da Empresa" (depende do que seu empregador faz)

**Frases-chave que te libertam:**
- "Excluindo trabalho feito inteiramente no tempo livre do Funcionario com os recursos proprios do Funcionario e nao relacionado ao negocio da Empresa" (esta e sua ressalva — muitos estados dos EUA exigem isso)
- Alguns estados (California, Washington, Minnesota, Illinois, outros) tem leis que limitam reivindicacoes de PI do empregador sobre projetos pessoais, independentemente do que diz o contrato.

### O Teste das 3 Perguntas

Para qualquer projeto de renda, pergunte:

1. **Tempo:** Voce esta fazendo esse trabalho no seu tempo livre? (Nao durante o horario de trabalho, nao durante turnos de sobreaviso)
2. **Equipamento:** Voce esta usando seu proprio hardware, sua propria internet, suas proprias chaves de API? (Nao o laptop do empregador, nao a VPN do empregador, nao as contas de nuvem do empregador)
3. **Assunto:** Isso nao esta relacionado ao negocio do seu empregador? (Se voce trabalha em uma empresa de IA para saude e quer vender servicos de IA para saude... isso e um problema. Se voce trabalha em uma empresa de IA para saude e quer vender processamento de documentos para corretores de imoveis... tudo bem.)

Se todas as tres respostas sao limpas, voce quase certamente esta bem. Se qualquer resposta e turva, esclareca antes de prosseguir.

> **Papo Reto:** A grande maioria dos desenvolvedores que fazem trabalho extra nunca tem problemas. Empregadores se preocupam em proteger vantagens competitivas, nao em impedir voce de ganhar dinheiro extra em projetos nao relacionados. Mas "quase certamente ok" nao e "definitivamente ok." Se seu contrato e incomumente amplo, tenha uma conversa com seu gerente ou RH — ou consulte um advogado. O lado negativo de nao verificar e muito pior que o leve constrangimento de perguntar.

### Passo 2: Escolha uma Estrutura Empresarial

Voce precisa de uma entidade legal para separar seus bens pessoais das suas atividades comerciais, e para abrir caminho para conta bancaria empresarial, processamento de pagamentos e beneficios fiscais.

{? if regional.country ?}
> **Sua localizacao: {= regional.country | fallback("Desconhecido") =}.** O tipo de entidade recomendado para sua regiao e uma **{= regional.business_entity_type | fallback("LLC ou equivalente") =}**, com custos tipicos de registro de {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Role ate a secao do seu pais abaixo, ou leia todas as secoes para entender como clientes em outras regioes operam.
{? endif ?}

{? if regional.country == "US" ?}
#### Estados Unidos (Sua Regiao)
{? else ?}
#### Estados Unidos
{? endif ?}

| Estrutura | Custo | Protecao | Melhor Para |
|-----------|-------|----------|-------------|
| **Sole Proprietorship** (padrao) | $0 | Nenhuma (responsabilidade pessoal) | Testando as aguas. Primeiros $1K. |
| **LLC de Membro Unico** | $50-500 (varia por estado) | Protecao de bens pessoais | Trabalho de renda ativo. A maioria dos desenvolvedores deve comecar aqui. |
| **Eleicao S-Corp** (sobre uma LLC) | Custo LLC + $0 pela eleicao | Mesma da LLC + beneficios fiscais de folha | Quando voce esta ganhando consistentemente $40K+/ano com isso |

**Recomendado para desenvolvedores americanos:** LLC de Membro Unico no seu estado de residencia.

**Estados mais baratos para formar:** Wyoming ($100, sem imposto de renda estadual), New Mexico ($50), Montana ($70). Mas formar no seu estado de residencia e geralmente o mais simples a menos que voce tenha um motivo especifico para nao fazer.

**Como registrar:**
1. Va ao site do Secretary of State do seu estado
2. Procure "form LLC" ou "business entity filing"
3. Preencha os Articles of Organization (formulario de 10 minutos)
4. Obtenha um EIN do IRS (gratuito, leva 5 minutos em irs.gov)

{? if regional.country == "GB" ?}
#### Reino Unido (Sua Regiao)
{? else ?}
#### Reino Unido
{? endif ?}

| Estrutura | Custo | Protecao | Melhor Para |
|-----------|-------|----------|-------------|
| **Sole Trader** | Gratis (registrar com HMRC) | Nenhuma | Primeira renda. Teste. |
| **Limited Company (Ltd)** | ~$15 via Companies House | Protecao de bens pessoais | Qualquer trabalho serio de renda. |

**Recomendado:** Ltd company via Companies House. Leva cerca de 20 minutos e custa GBP 12.

#### Uniao Europeia

Varia significativamente por pais, mas o padrao geral:

- **Alemanha:** Einzelunternehmer (empresa individual) para comecar, GmbH para trabalho serio (mas GmbH requer EUR 25.000 de capital — considere UG por EUR 1)
- **Holanda:** Eenmanszaak (empresa individual, registro gratis) ou BV (comparavel a Ltd)
- **Franca:** Micro-entrepreneur (simplificado, recomendado para comecar)
- **Estonia:** e-Residency + OUE (popular para nao residentes, totalmente online)

{? if regional.country == "AU" ?}
#### Australia (Sua Regiao)
{? else ?}
#### Australia
{? endif ?}

| Estrutura | Custo | Protecao | Melhor Para |
|-----------|-------|----------|-------------|
| **Sole Trader** | ABN gratuito | Nenhuma | Para comecar |
| **Pty Ltd** | ~AUD 500-800 via ASIC | Protecao de bens pessoais | Renda seria |

**Recomendado:** Comece com um ABN de Sole Trader (gratuito, instantaneo), mude para Pty Ltd quando estiver ganhando consistentemente.

### Passo 3: Processamento de Pagamentos (setup de 15 minutos)

Voce precisa de um jeito de receber pagamentos. Configure isso agora, nao quando seu primeiro cliente estiver esperando.

{? if regional.payment_processors ?}
> **Recomendado para {= regional.country | fallback("sua regiao") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (recomendado para a maioria dos desenvolvedores):**

```
1. Va a stripe.com
2. Crie uma conta com seu email empresarial
3. Complete a verificacao de identidade
4. Conecte sua conta bancaria empresarial
5. Agora voce pode aceitar pagamentos, criar faturas e configurar assinaturas
```

Tempo: ~15 minutos. Voce pode comecar a aceitar pagamentos imediatamente (Stripe retém fundos por 7 dias em contas novas).

**Lemon Squeezy (recomendado para produtos digitais):**

Se voce esta vendendo produtos digitais (templates, ferramentas, cursos, SaaS), o Lemon Squeezy atua como seu Merchant of Record. Isso significa:
- Eles cuidam de imposto sobre vendas, IVA e GST para voce globalmente
- Voce nao precisa se registrar para IVA na UE
- Eles cuidam de reembolsos e disputas

```
1. Va a lemonsqueezy.com
2. Crie uma conta
3. Configure sua loja
4. Adicione produtos
5. Eles cuidam de todo o resto
```

**Stripe Atlas (para desenvolvedores internacionais ou quem quer uma entidade nos EUA):**

Se voce esta fora dos EUA mas quer vender para clientes americanos com uma entidade nos EUA:
- $500 taxa unica
- Cria uma LLC em Delaware para voce
- Configura uma conta bancaria nos EUA (via Mercury ou Stripe)
- Fornece servico de agente registrado
- Leva cerca de 1-2 semanas

### Passo 4: Politica de Privacidade e Termos de Servico

Se voce esta vendendo qualquer servico ou produto online, voce precisa disso. Nao pague um advogado por boilerplate.

**Fontes gratuitas e confiaveis para templates:**
- **Termly.io** — Gerador gratuito de politica de privacidade e ToS. Responda perguntas, receba documentos.
- **Avodocs.com** — Documentos legais open-source para startups. Gratuitos.
- **choosealicense.com do GitHub** — Para licencas de projetos open-source especificamente.
- **Politicas open-source do Basecamp** — Procure "Basecamp open source policies" — bons templates em linguagem simples.

**O que sua politica de privacidade deve cobrir (se voce processa dados de clientes):**
- Quais dados voce coleta
- Como voce os processa (localmente — esta e sua vantagem)
- Por quanto tempo voce os retém
- Como clientes podem solicitar exclusao
- Se terceiros acessam os dados (idealmente: nenhum)

**Tempo:** 30 minutos com um gerador de templates. Feito.

### Passo 5: Conta Bancaria Separada

Nao passe renda empresarial pela sua conta corrente pessoal. As razoes:

1. **Clareza fiscal:** Quando chegar a hora dos impostos, voce precisa saber exatamente o que era renda empresarial e o que nao era.
2. **Protecao legal:** Se voce tem uma LLC, misturar fundos pessoais e empresariais pode "furar o veu corporativo" — significando que um tribunal pode ignorar a protecao de responsabilidade da sua LLC.
3. **Profissionalismo:** Faturas de "Consultoria do Joao Ltda" chegando em uma conta empresarial dedicada parece legitimo. Pagamentos no seu PIX pessoal nao.

**Conta bancaria empresarial gratuita ou de baixo custo:**
{? if regional.country == "US" ?}
- **Mercury** (recomendado para voce) — Gratuito, projetado para startups. Excelente API se voce quiser automatizar contabilidade depois.
- **Relay** — Gratuito, bom para separar fluxos de renda em sub-contas.
{? elif regional.country == "GB" ?}
- **Starling Bank** (recomendado para voce) — Conta empresarial gratuita, configuracao instantanea.
- **Wise Business** — Multi-moeda de baixo custo. Otimo se voce atende clientes internacionais.
{? else ?}
- **Mercury** (EUA) — Gratuito, projetado para startups. Excelente API se voce quiser automatizar contabilidade depois.
- **Relay** (EUA) — Gratuito, bom para separar fluxos de renda em sub-contas.
- **Starling Bank** (UK) — Conta empresarial gratuita.
{? endif ?}
- **Wise Business** (Internacional) — Multi-moeda de baixo custo. Otimo para receber pagamentos em USD, EUR, GBP, etc.
- **Qonto** (UE) — Conta bancaria empresarial limpa para empresas europeias.

Abra a conta agora. Leva 10-15 minutos online e 1-3 dias para verificacao.

### Passo 6: Basico de Impostos para Renda Extra de Desenvolvedor

{? if regional.tax_note ?}
> **Nota fiscal para {= regional.country | fallback("sua regiao") =}:** {= regional.tax_note | fallback("Consulte um profissional fiscal local para detalhes.") =}
{? endif ?}

> **Papo Reto:** Impostos sao a coisa que a maioria dos desenvolvedores ignora ate abril, e entao entra em panico. Gastar 30 minutos agora te economiza dinheiro real e estresse.

**Estados Unidos:**
- Renda extra acima de $400/ano requer imposto de trabalho autonomo (~15.3% para Social Security + Medicare)
- Mais a aliquota normal de imposto de renda sobre o lucro liquido
- **Impostos estimados trimestrais:** Se voce vai dever mais de $1.000 em impostos, o IRS espera pagamentos trimestrais (15 de abril, 15 de junho, 15 de setembro, 15 de janeiro). Pagamento insuficiente gera multas.
- Reserve **25-30%** da renda liquida para impostos. Coloque em uma conta poupanca separada imediatamente.

**Deducoes comuns para renda extra de desenvolvedores:**
- Custos de API (OpenAI, Anthropic, etc.) — 100% dedutiveis
- Compras de hardware usados para o negocio — depreciaveis ou deducao Secao 179
- Custo de eletricidade atribuivel ao uso comercial
- Assinaturas de software usadas para trabalho de renda
- Deducao de escritorio domestico (simplificada: $5/sq ft, ate 300 sq ft = $1.500)
- Internet (percentual de uso comercial)
- Nomes de dominio, hospedagem, servicos de email
- Desenvolvimento profissional (cursos, livros) relacionados ao seu trabalho de renda

**Reino Unido:**
- Declaracao via Self Assessment
- Renda comercial abaixo de GBP 1.000: isenta (Trading Allowance)
- Acima disso: pague Income Tax + Class 4 NICs sobre lucros
- Datas de pagamento: 31 de janeiro e 31 de julho

**Rastreie tudo desde o primeiro dia.** Use uma planilha simples se nada mais:

```
| Data       | Categoria   | Descricao              | Valor   | Tipo    |
|------------|-------------|------------------------|---------|---------|
| 2025-01-15 | API         | Credito Anthropic      | -$20.00 | Despesa |
| 2025-01-18 | Receita     | Fatura cliente #001    | +$500.00| Entrada |
| 2025-01-20 | Software    | Plano Vercel Pro       | -$20.00 | Despesa |
| 2025-01-20 | Reserva Tax | 30% da renda liquida   | -$138.00| Transf. |
```

> **Erro Comum:** "Vou resolver os impostos depois." Depois e o Q4, voce deve $3.000 em impostos estimados mais multas, e voce ja gastou o dinheiro. Automatize: toda vez que renda chegar na sua conta empresarial, transfira 30% para uma conta poupanca de impostos imediatamente.

### Checkpoint Licao 4

Voce agora deve ter (ou ter um plano para):
- [ ] Lido a clausula de PI do seu contrato de trabalho
- [ ] Passado no Teste das 3 Perguntas para seu trabalho de renda planejado
- [ ] Escolhido uma estrutura empresarial (ou decidido comecar como autonomo)
- [ ] Processamento de pagamentos configurado (Stripe ou Lemon Squeezy)
- [ ] Politica de privacidade e ToS de um gerador de templates
- [ ] Conta bancaria empresarial separada (ou solicitacao enviada)
- [ ] Estrategia fiscal: reserva de 30% + calendario de pagamentos trimestrais

*No curso STREETS completo, o Modulo E (Playbook de Execucao) inclui templates de modelagem financeira que calculam automaticamente suas obrigacoes fiscais, rentabilidade do projeto e pontos de equilibrio para cada motor de receita.*

---

## Licao 5: O Orcamento de {= regional.currency_symbol | fallback("$") =}200/mes

*"Seu negocio tem um burn rate. Conheca-o. Controle-o. Faca-o render."*

### Por Que {= regional.currency_symbol | fallback("$") =}200/mes

Duzentos {= regional.currency | fallback("dolares") =} por mes e o orcamento minimo viavel para uma operacao de renda de desenvolvedor. E o suficiente para rodar servicos reais, atender clientes reais e gerar receita real. Tambem e pequeno o bastante que se nada funcionar, voce nao apostou tudo.

O objetivo e simples: **transformar {= regional.currency_symbol | fallback("$") =}200/mes em {= regional.currency_symbol | fallback("$") =}600+/mes em 90 dias.** Se voce conseguir, voce tem um negocio. Se nao, voce muda a estrategia — nao aumenta o orcamento.

### A Divisao do Orcamento

#### Nivel 1: Creditos de API — $50-100/mes

Este e seu compute de producao para qualidade voltada ao cliente.

**Alocacao inicial recomendada:**

```
Anthropic (Claude):     $40/mes  — Seu principal para saida de qualidade
OpenAI (GPT-4o-mini):   $20/mes  — Trabalho de volume barato, fallback
DeepSeek:               $10/mes  — Tarefas economicas, experimentacao
Buffer:                 $30/mes  — Excedente ou teste de novos provedores
```

**A estrategia de gasto hibrida:**
- Use LLMs locais para 80% do processamento (classificacao, extracao, resumo, rascunhos)
- Use chamadas de API para 20% do processamento (passo final de qualidade, raciocinio complexo, saida voltada ao cliente)
- Seu custo efetivo por tarefa cai drasticamente vs. uso puro de API

{? if computed.monthly_electricity_estimate ?}
> **Seu custo de eletricidade estimado:** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/mes para operacao 24/7 a {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh. Isso ja esta incluido no seu custo operacional efetivo.
{? endif ?}

#### Nivel 2: Infraestrutura — {= regional.currency_symbol | fallback("$") =}30-50/mes

```
Nome de dominio:        $12/ano ($1/mes)     — Namecheap, Cloudflare, Porkbun
Email (empresarial):    $0-6/mes             — Zoho Mail gratuito, ou Google Workspace $6
VPS (opcional):         $5-20/mes            — Para hospedar servicos leves
                                                Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/mes               — Cloudflare plano gratuito
Hospedagem (estatica):  $0/mes               — Vercel, Netlify, Cloudflare Pages (planos gratuitos)
```

#### Nivel 3: Ferramentas — {= regional.currency_symbol | fallback("$") =}20-30/mes

```
Analytics:              $0/mes    — Plausible Cloud ($9) ou self-hosted,
                                    ou Vercel Analytics (plano gratuito)
                                    ou apenas Cloudflare analytics (gratuito)
Email marketing:        $0/mes    — Buttondown (gratuito ate 100 inscritos),
                                    Resend ($0 para 3K emails/mes)
Monitoramento:          $0/mes    — UptimeRobot (gratuito, 50 monitores),
                                    Better Stack (plano gratuito)
Design:                 $0/mes    — Figma (gratuito), Canva (plano gratuito)
Contabilidade:          $0/mes    — Wave (gratuito), ou uma planilha
                                    Hledger (gratuito, contabilidade em texto puro)
```

> **Papo Reto:** Voce pode rodar todo seu stack de ferramentas em planos gratuitos quando esta comecando. Os $20-30 alocados aqui sao para quando voce ultrapassar os planos gratuitos ou quiser um recurso premium especifico. Nao gaste so porque esta no orcamento. Orcamento nao gasto e lucro.

#### Nivel 4: Reserva — {= regional.currency_symbol | fallback("$") =}0-30/mes

Este e seu fundo de "coisas que nao previ":
- Um pico de custo de API de um job em lote inesperadamente grande
- Uma ferramenta que voce precisa para um projeto especifico de cliente
- Compra de dominio de emergencia quando voce encontra o nome perfeito
- Uma compra unica (tema, template, conjunto de icones)

Se voce nao usar a reserva, ela acumula. Apos 3 meses de reserva nao utilizada, considere realocar para creditos de API ou infraestrutura.

### O Calculo do ROI

Este e o unico numero que importa:

```
Receita Mensal - Custos Mensais = Lucro Liquido
Lucro Liquido / Custos Mensais = Multiplo de ROI

Exemplo:
$600 receita - $200 custos = $400 lucro
$400 / $200 = 2x ROI

A meta: 3x ROI ($600+ receita sobre $200 de gasto)
O minimo: 1x ROI ($200 receita = empate)
Abaixo de 1x: Mude a estrategia ou reduza custos
```

{@ insight cost_projection @}

**Quando aumentar o orcamento:**

Aumente seu orcamento APENAS quando:
1. Voce esta consistentemente em 2x+ ROI por 2+ meses
2. Mais gasto aumentaria diretamente a receita (ex., mais creditos de API = mais capacidade de atendimento)
3. O aumento esta ligado a um fluxo de renda especifico e testado

**Quando NAO aumentar o orcamento:**
- "Acho que essa nova ferramenta vai ajudar" (teste alternativas gratuitas primeiro)
- "Todo mundo diz que voce precisa gastar dinheiro para ganhar dinheiro" (nao neste estagio)
- "Um VPS maior vai tornar meu servico mais rapido" (velocidade e realmente o gargalo?)
- Voce ainda nao atingiu 1x ROI (corrija a receita, nao o gasto)

**A escada de crescimento:**

```
$200/mes  → Provando o conceito (meses 1-3)
$500/mes  → Escalando o que funciona (meses 4-6)
$1000/mes → Multiplos fluxos de renda (meses 6-12)
$2000+/mes → Operacao empresarial completa (ano 2+)

Cada passo requer provar o ROI no nivel atual primeiro.
```

> **Erro Comum:** Tratar os {= regional.currency_symbol | fallback("$") =}200 como um "investimento" que nao precisa retornar dinheiro imediatamente. Nao. Isto e um experimento com prazo de 90 dias. Se {= regional.currency_symbol | fallback("$") =}200/mes nao geram {= regional.currency_symbol | fallback("$") =}200/mes em receita dentro de 90 dias, algo na estrategia precisa mudar. O dinheiro, o mercado, a oferta — algo nao esta funcionando. Seja honesto consigo mesmo.

### Checkpoint Licao 5

Voce agora deve ter:
- [ ] Um orcamento mensal de ~$200 alocado em quatro niveis
- [ ] Contas de API criadas com limites de gasto configurados
- [ ] Decisoes de infraestrutura tomadas (apenas local vs. local + VPS)
- [ ] Um stack de ferramentas selecionado (principalmente planos gratuitos para comecar)
- [ ] Metas de ROI: 3x em 90 dias
- [ ] Uma regra clara: aumente o orcamento apenas apos provar o ROI

*No curso STREETS completo, o Modulo E (Playbook de Execucao) inclui um template de dashboard financeira que rastreia seu gasto, receita e ROI por motor de receita em tempo real — para que voce sempre saiba quais fluxos sao lucrativos e quais precisam de ajuste.*

---

## Licao 6: Seu Documento do Stack Soberano

*"Todo negocio tem um plano. Este e o seu — e cabe em duas paginas."*

### O Entregavel

Esta e a coisa mais importante que voce criara no Modulo S. Seu Documento do Stack Soberano e uma referencia unica que captura tudo sobre sua infraestrutura de geracao de renda. Voce vai referencia-lo ao longo de todo o curso STREETS, atualiza-lo conforme sua configuracao evolui, e usa-lo para tomar decisoes lucidas sobre o que construir e o que pular.

Crie um novo arquivo. Markdown, Google Doc, pagina do Notion, texto puro — o que voce realmente vai manter. Use o template abaixo, preenchendo cada campo com os numeros e decisoes das Licoes 1-5.

### O Template

{? if computed.profile_completeness != "0" ?}
> **Vantagem inicial:** O 4DA ja detectou algumas das suas especificacoes de hardware e informacoes do stack. Procure as dicas pre-preenchidas abaixo — vao te economizar tempo preenchendo o template.
{? endif ?}

Copie o template inteiro e preencha. Cada campo. Sem pular nada.

```markdown
# Documento do Stack Soberano
# [Seu Nome ou Nome do Negocio]
# Criado: [Data]
# Ultima Atualizacao: [Data]

---

## 1. INVENTARIO DE HARDWARE

### Maquina Principal
- **Tipo:** [Desktop / Laptop / Mac / Servidor]
- **CPU:** [Modelo] — [X] nucleos, [X] threads
- **RAM:** [X] GB [DDR4/DDR5]
- **GPU:** [Modelo] — [X] GB VRAM (ou "Nenhuma — inferencia apenas CPU")
- **Armazenamento:** [X] GB SSD livre / [X] GB total
- **SO:** [Distro Linux / versao macOS / versao Windows]

### Rede
- **Download:** [X] Mbps
- **Upload:** [X] Mbps
- **Latencia para APIs de nuvem:** [X] ms
- **Confiabilidade do ISP:** [Estavel / Quedas ocasionais / Instavel]

### Capacidade de Uptime
- **Pode rodar 24/7:** [Sim / Nao — motivo]
- **Nobreak:** [Sim / Nao]
- **Acesso remoto:** [SSH / RDP / Tailscale / Nenhum]

### Custo Mensal de Infraestrutura
- **Eletricidade (estimativa 24/7):** $[X]/mes
- **Internet:** $[X]/mes (porcao comercial)
- **Custo fixo total de infraestrutura:** $[X]/mes

---

## 2. STACK LLM

### Modelos Locais (via Ollama)
| Modelo | Tamanho | Tokens/seg | Caso de Uso |
|--------|---------|-----------|-------------|
| [ex., llama3.1:8b] | [X]B | [X] tok/s | [ex., Classificacao, extracao] |
| [ex., mistral:7b] | [X]B | [X] tok/s | [ex., Resumo, rascunhos] |
| [ex., deepseek-coder] | [X]B | [X] tok/s | [ex., Geracao de codigo] |

### Modelos de API (para saida de qualidade critica)
| Provedor | Modelo | Orcamento Mensal | Caso de Uso |
|----------|--------|-----------------|-------------|
| [ex., Anthropic] | [Claude 3.5 Sonnet] | $[X] | [ex., Conteudo voltado ao cliente] |
| [ex., OpenAI] | [GPT-4o-mini] | $[X] | [ex., Fallback de processamento de volume] |

### Estrategia de Inferencia
- **Local lida com:** [X]% das requisicoes ([lista de tarefas])
- **API lida com:** [X]% das requisicoes ([lista de tarefas])
- **Custo misto estimado por 1M tokens:** $[X]

---

## 3. ORCAMENTO MENSAL

| Categoria | Alocacao | Real (atualize mensalmente) |
|-----------|---------|---------------------------|
| Creditos de API | $[X] | $[  ] |
| Infraestrutura (VPS, dominio, email) | $[X] | $[  ] |
| Ferramentas (analytics, email marketing) | $[X] | $[  ] |
| Reserva | $[X] | $[  ] |
| **Total** | **$[X]** | **$[  ]** |

### Meta de Receita
- **Mes 1-3:** $[X]/mes (minimo: cobrir custos)
- **Mes 4-6:** $[X]/mes
- **Mes 7-12:** $[X]/mes

---

## 4. STATUS LEGAL

- **Status de emprego:** [Empregado / Freelancer / Entre empregos]
- **Clausula de PI revisada:** [Sim / Nao / N/A]
- **Nivel de risco da clausula de PI:** [Limpa / Turva — precisa revisao / Restritiva]
- **Entidade empresarial:** [LLC / Ltd / Autonomo / Nenhuma ainda]
  - **Estado/Pais:** [Onde registrada]
  - **CNPJ/CPF:** [Obtido / Pendente / Nao necessario ainda]
- **Processamento de pagamentos:** [Stripe / Lemon Squeezy / Outro] — [Ativo / Pendente]
- **Conta bancaria empresarial:** [Aberta / Pendente / Usando pessoal (corrija isso)]
- **Politica de privacidade:** [Feita / Ainda nao — URL: ___]
- **Termos de servico:** [Feitos / Ainda nao — URL: ___]

---

## 5. INVENTARIO DE TEMPO

- **Horas disponiveis por semana para projetos de renda:** [X] horas
  - **Manhas de dias uteis:** [X] horas
  - **Noites de dias uteis:** [X] horas
  - **Fins de semana:** [X] horas
- **Fuso horario:** [Seu fuso horario]
- **Melhores blocos de trabalho profundo:** [ex., "Sabado 6h-12h, noites de dias uteis 20h-22h"]

### Plano de Alocacao de Tempo
| Atividade | Horas/semana |
|-----------|-------------|
| Construcao/codigo | [X] |
| Marketing/vendas | [X] |
| Trabalho para cliente/entrega | [X] |
| Aprendizado/experimentacao | [X] |
| Admin (faturamento, email, etc.) | [X] |

> Regra: Nunca aloque mais de 70% do tempo disponivel.
> A vida acontece. Burnout e real. Deixe um buffer.

---

## 6. INVENTARIO DE HABILIDADES

### Habilidades Principais (coisas que voce poderia ensinar a outros)
1. [Habilidade] — [anos de experiencia]
2. [Habilidade] — [anos de experiencia]
3. [Habilidade] — [anos de experiencia]

### Habilidades Secundarias (competente mas nao especialista)
1. [Habilidade]
2. [Habilidade]
3. [Habilidade]

### Explorando (aprendendo agora ou quer aprender)
1. [Habilidade]
2. [Habilidade]

### Combinacoes Unicas
O que torna SUA combinacao de habilidades incomum? (Isso se torna sua fossa no Modulo T)
- [ex., "Conheco Rust E padroes de dados de saude — pouquissimas pessoas tem ambos"]
- [ex., "Consigo construir apps full-stack E entendo logistica de supply chain de uma carreira anterior"]
- [ex., "Sou fluente em 3 idiomas E sei programar — posso atender mercados nao anglófonos que a maioria das ferramentas dev ignora"]

---

## 7. RESUMO DO STACK SOBERANO

### O Que Posso Oferecer Hoje
(Baseado em hardware + habilidades + tempo, o que voce poderia vender ESTA SEMANA se alguem pedisse?)
1. [ex., "Processamento local de documentos — extracao de dados de PDFs com privacidade"]
2. [ex., "Scripts de automacao personalizados para [dominio especifico]"]
3. [ex., "Escrita tecnica / documentacao"]

### O Que Estou Construindo
(Baseado no framework STREETS completo — preencha conforme avanca no curso)
1. [Motor de Receita 1 — do Modulo R]
2. [Motor de Receita 2 — do Modulo R]
3. [Motor de Receita 3 — do Modulo R]

### Restricoes-Chave
(Seja honesto — estas nao sao fraquezas, sao parametros)
- [ex., "Apenas 10 horas/semana disponiveis"]
- [ex., "Sem GPU — inferencia apenas CPU, vou depender de APIs para tarefas LLM"]
- [ex., "Contrato de trabalho e restritivo — preciso ficar em dominios nao relacionados"]
- [ex., "Nao baseado nos EUA — algumas opcoes de pagamento/legais sao limitadas"]

---

*Este documento e uma referencia viva. Atualize mensalmente.*
*Proxima data de revisao: [Data + 30 dias]*
```

{? if dna.primary_stack ?}
> **Pre-preenchimento do seu Developer DNA:**
> - **Stack principal:** {= dna.primary_stack | fallback("Nao detectado") =}
> - **Interesses:** {= dna.interests | fallback("Nao detectados") =}
> - **Resumo de identidade:** {= dna.identity_summary | fallback("Ainda nao perfilado") =}
{? if dna.blind_spots ?}> - **Pontos cegos para observar:** {= dna.blind_spots | fallback("Nenhum detectado") =}
{? endif ?}
{? elif stack.primary ?}
> **Pre-preenchimento do stack detectado:** Suas tecnologias principais sao {= stack.primary | fallback("ainda nao detectadas") =}. {? if stack.adjacent ?}Habilidades adjacentes: {= stack.adjacent | fallback("nenhuma detectada") =}.{? endif ?} Use estas para preencher o Inventario de Habilidades acima.
{? endif ?}

{@ insight t_shape @}

### Como Usar Este Documento

1. **Antes de iniciar qualquer novo projeto:** Verifique seu Stack Soberano. Voce tem o hardware, tempo, habilidades e orcamento para executar?
2. **Antes de comprar qualquer coisa:** Verifique sua alocacao de orcamento. Esta compra esta no plano?
3. **Revisao mensal:** Atualize a coluna "Real" no seu orcamento. Atualize numeros de receita. Ajuste alocacoes baseado no que esta funcionando.
4. **Quando alguem perguntar o que voce faz:** Sua secao "O Que Posso Oferecer Hoje" e seu pitch instantaneo.
5. **Quando voce estiver tentado a perseguir uma nova ideia brilhante:** Verifique suas restricoes. Cabe no seu tempo, habilidades e hardware? Se nao, adicione a "O Que Estou Construindo" para depois.

### O Exercicio de Uma Hora

Configure um timer para 60 minutos. Preencha cada campo do template. Nao pense demais. Nao pesquise extensivamente. Escreva o que voce sabe agora. Voce pode atualizar depois.

Os campos que voce nao consegue preencher? Esses sao seus itens de acao para esta semana:
- Numeros de benchmark vazios? Execute o script de benchmark da Licao 2.
- Nenhuma entidade empresarial? Inicie o processo de registro da Licao 4.
- Nenhum processamento de pagamentos? Configure o Stripe da Licao 4.
- Inventario de habilidades vazio? Gaste 15 minutos listando tudo pelo que voce foi pago nos ultimos 5 anos.

> **Erro Comum:** Gastar 3 horas tornando o documento "perfeito" em vez de 1 hora tornando-o "feito." O Documento do Stack Soberano e uma referencia de trabalho, nao um plano de negocios para investidores. Ninguem vai ve-lo alem de voce. Precisao importa. Formatacao nao.

### Checkpoint Licao 6

Voce agora deve ter:
- [ ] Um Documento do Stack Soberano completo salvo em algum lugar que voce realmente vai abrir
- [ ] Todas as seis secoes preenchidas com numeros reais (nao aspiracionais)
- [ ] Uma lista clara de itens de acao para lacunas na sua configuracao
- [ ] Uma data definida para sua primeira revisao mensal (30 dias a partir de agora)

---

## Modulo S: Completo

{? if progress.completed("MODULE_S") ?}
> **Modulo S completo.** Voce terminou {= progress.completed_count | fallback("1") =} de {= progress.total_count | fallback("7") =} modulos STREETS. {? if progress.completed_modules ?}Completos: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### O Que Voce Construiu em Duas Semanas

Olhe o que voce agora tem que nao tinha quando comecou:

1. **Um inventario de hardware** mapeado para capacidades de geracao de renda — nao apenas especificacoes em um adesivo.
2. **Um stack LLM local de nivel producao** com Ollama, testado no seu hardware real, configurado para cargas de trabalho reais.
3. **Uma vantagem de privacidade** que voce sabe como comercializar — com linguagem especifica para audiencias especificas.
4. **Uma base legal e financeira** — entidade empresarial (ou plano), processamento de pagamentos, conta bancaria, estrategia fiscal.
5. **Um orcamento controlado** com metas de ROI claras e um prazo de 90 dias para provar o modelo.
6. **Um Documento do Stack Soberano** que captura tudo acima em uma unica referencia que voce usara para cada decisao futura.

Isso e mais do que a maioria dos desenvolvedores jamais configura. Serio. A maioria das pessoas que quer gerar renda extra pula direto para "construir algo legal" e depois se pergunta por que nao consegue receber pagamento. Agora voce tem a infraestrutura para receber pagamento.

Mas infraestrutura sem direcao e apenas um hobby caro. Voce precisa saber para onde apontar esse stack.

{@ temporal market_timing @}

### O Que Vem Depois: Modulo T — Fossas Tecnicas

O Modulo S te deu a fundacao. O Modulo T responde a pergunta critica: **como voce constroi algo que concorrentes nao podem facilmente copiar?**

Aqui esta o que o Modulo T cobre:

- **Pipelines de dados proprietarias** — como criar datasets que so voce tem acesso, legal e eticamente
- **Configuracoes de modelo personalizadas** — fine-tuning e prompt engineering que produzem qualidade de saida que outros nao conseguem igualar com configuracoes padrao
- **Stacks de habilidades que compoem** — por que "Python + saude" bate "Python + JavaScript" para renda, e como identificar sua combinacao unica
- **Barreiras tecnicas de entrada** — designs de infraestrutura que levariam meses para um concorrente replicar
- **A Auditoria de Fossa** — um framework para avaliar se seu projeto tem uma vantagem defensavel ou e apenas mais um servico commodity

A diferenca entre um desenvolvedor que ganha $500/mes e um que ganha $5.000/mes raramente e habilidade. Sao as fossas. Coisas que tornam sua oferta dificil de replicar, mesmo que alguem tenha o mesmo hardware e os mesmos modelos.

### A Roadmap STREETS Completa

| Modulo | Titulo | Foco | Duracao |
|--------|--------|------|---------|
| **S** | Configuracao Soberana | Infraestrutura, legal, orcamento | Semanas 1-2 (completo) |
| **T** | Fossas Tecnicas | Vantagens defensaveis, ativos proprietarios | Semanas 3-4 |
| **R** | Motores de Receita | Playbooks de monetizacao especificos com codigo | Semanas 5-8 |
| **E** | Playbook de Execucao | Sequencias de lancamento, precificacao, primeiros clientes | Semanas 9-10 |
| **E** | Vantagem em Evolucao | Manter-se a frente, deteccao de tendencias, adaptacao | Semanas 11-12 |
| **T** | Automacao Tatica | Automatizando operacoes para renda passiva | Semanas 13-14 |
| **S** | Empilhamento de Fluxos | Multiplas fontes de renda, estrategia de portfolio | Semanas 15-16 |

O Modulo R (Motores de Receita) e onde a maior parte do dinheiro e feita. Mas sem S e T, voce esta construindo na areia.

---

**Pronto para o playbook completo?**

Voce viu a fundacao. Voce mesmo a construiu. Agora obtenha o sistema completo.

**Obtenha STREETS Core** — o curso completo de 16 semanas com todos os sete modulos, templates de codigo para motores de receita, dashboards financeiras e a comunidade privada de desenvolvedores construindo renda nos seus proprios termos.
