# Modulo S: Configuracion Soberana

**Curso STREETS de Ingresos para Desarrolladores — Modulo Gratuito**
*Semanas 1-2 | 6 Lecciones | Entregable: Tu Documento de Stack Soberano*

> "Tu equipo es tu infraestructura de negocio. Configuralo como tal."

---

Ya eres dueno de la herramienta generadora de ingresos mas poderosa que la mayoria de las personas jamas tendra: una estacion de trabajo de desarrollador con conexion a internet, computo local y las habilidades para conectar todo.

La mayoria de los desarrolladores tratan su equipo como un producto de consumo. Algo en lo que juegan, programan, navegan. Pero esa misma maquina — la que esta debajo de tu escritorio ahora mismo — puede ejecutar inferencia, servir APIs, procesar datos y generar ingresos las 24 horas del dia mientras duermes.

Este modulo trata de ver lo que ya tienes a traves de un lente diferente. No "que puedo construir?" sino "que puedo vender?"

Al final de estas dos semanas, tendras:

- Un inventario claro de tus capacidades generadoras de ingresos
- Un stack de LLM local de grado produccion
- Una base legal y financiera (aunque sea minima)
- Un Documento de Stack Soberano escrito que se convierte en el plano de tu negocio

Nada de rodeos. Nada de "solo cree en ti mismo." Numeros reales, comandos reales, decisiones reales.

{@ mirror sovereign_readiness @}

Comencemos.

---

## Leccion 1: La Auditoria del Equipo

*"No necesitas una 4090. Esto es lo que realmente importa."*

### Tu Maquina Es un Activo de Negocio

Cuando una empresa evalua su infraestructura, no solo lista especificaciones — mapea capacidades a oportunidades de ingreso. Eso es exactamente lo que vas a hacer ahora.

{? if computed.profile_completeness != "0" ?}
> **Tu Equipo Actual:** {= profile.cpu.model | fallback("Unknown CPU") =} ({= profile.cpu.cores | fallback("?") =} nucleos / {= profile.cpu.threads | fallback("?") =} hilos), {= profile.ram.total | fallback("?") =} {= profile.ram.type | fallback("") =} RAM, {= profile.gpu.model | fallback("No dedicated GPU") =} {? if profile.gpu.exists ?}({= profile.gpu.vram | fallback("?") =} VRAM){? endif ?}, {= profile.storage.free | fallback("?") =} libres / {= profile.storage.total | fallback("?") =} totales ({= profile.storage.type | fallback("unknown") =}), ejecutando {= profile.os.name | fallback("unknown OS") =} {= profile.os.version | fallback("") =}.
{? endif ?}

Abre una terminal y ejecuta lo siguiente. Anota cada numero. Los necesitaras para tu Documento de Stack Soberano en la Leccion 6.

### Inventario de Hardware

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

**Lo que importa para los ingresos:**
- La cantidad de nucleos determina cuantas tareas concurrentes puede manejar tu equipo. Ejecutar un LLM local mientras procesas un trabajo por lotes simultaneamente requiere paralelismo real.
{? if profile.cpu.cores ?}
- *Tu {= profile.cpu.model | fallback("CPU") =} tiene {= profile.cpu.cores | fallback("?") =} nucleos — revisa la tabla de requisitos a continuacion para ver que motores de ingresos soporta tu CPU.*
{? endif ?}
- Para la mayoria de los motores de ingresos en este curso, cualquier CPU moderno de 8+ nucleos de los ultimos 5 anos es suficiente.
- Si estas ejecutando LLMs locales solo con CPU (sin GPU), necesitas 16+ nucleos. Un Ryzen 7 5800X o Intel i7-12700 es el piso practico.

#### RAM

```bash
# Linux
free -h

# macOS
sysctl -n hw.memsize | awk '{print $0/1073741824 " GB"}'

# Windows (PowerShell)
(Get-CimInstance -ClassName Win32_ComputerSystem).TotalPhysicalMemory / 1GB
```

**Lo que importa para los ingresos:**
- 16 GB: Minimo indispensable. Puedes ejecutar modelos 7B y hacer trabajo basico de automatizacion.
- 32 GB: Comodo. Ejecuta modelos 13B localmente, maneja multiples proyectos, manten tu entorno de desarrollo funcionando junto con cargas de trabajo de ingresos.
- 64 GB+: Puedes ejecutar modelos 30B+ en CPU, o mantener multiples modelos cargados. Aqui es donde las cosas se ponen interesantes para vender servicios de inferencia.
{? if profile.ram.total ?}
*Tu sistema tiene {= profile.ram.total | fallback("?") =} RAM. Revisa la tabla anterior para ver en que nivel de capacidad estas — esto afecta directamente que modelos locales son practicos para tus cargas de trabajo de ingresos.*
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

**Lo que importa para los ingresos:**

Esta es la especificacion con la que la gente se obsesiona, y aqui esta la verdad honesta: **tu GPU determina tu nivel de LLM local, y tu nivel de LLM local determina que flujos de ingresos funcionan mas rapido.** Pero no determina si puedes ganar dinero o no.

| VRAM | Capacidad LLM | Relevancia para Ingresos |
|------|---------------|--------------------------|
| 0 (solo CPU) | Modelos 7B a ~5 tokens/seg | Procesamiento por lotes, trabajo asincrono. Lento pero funcional. |
| 6-8 GB (RTX 3060, etc.) | Modelos 7B a ~30 tok/seg, 13B cuantizado | Suficiente para la mayoria de flujos de ingresos por automatizacion. |
| 12 GB (RTX 3060 12GB, 4070) | 13B a velocidad completa, 30B cuantizado | Punto optimo. La mayoria de los motores de ingresos funcionan bien aqui. |
| 16-24 GB (RTX 4090, 3090) | Modelos 30B-70B | Nivel premium. Vende calidad que otros no pueden igualar localmente. |
| 48 GB+ (GPU dual, A6000) | 70B+ a velocidad | Inferencia local de grado empresarial. Ventaja competitiva seria. |
| Apple Silicon 32GB+ (M2/M3 Pro/Max) | 30B+ usando memoria unificada | Excelente eficiencia. Menor costo energetico que el equivalente NVIDIA. |

{@ insight hardware_benchmark @}

{? if profile.gpu.exists ?}
> **Tu GPU:** {= profile.gpu.model | fallback("Unknown") =} con {= profile.gpu.vram | fallback("?") =} VRAM — {? if computed.gpu_tier == "premium" ?}estas en el nivel premium. Modelos 30B-70B estan a tu alcance localmente. Esta es una ventaja competitiva seria.{? elif computed.gpu_tier == "sweet_spot" ?}estas en el punto optimo. 13B a velocidad completa, 30B cuantizado. La mayoria de los motores de ingresos funcionan bien aqui.{? elif computed.gpu_tier == "capable" ?}puedes ejecutar modelos 7B a buena velocidad y 13B cuantizado. Suficiente para la mayoria de flujos de ingresos por automatizacion.{? else ?}tienes aceleracion por GPU disponible. Revisa la tabla anterior para ver donde te ubicas.{? endif ?}
{? else ?}
> **No se detecto GPU dedicada.** Ejecutaras inferencia en CPU, lo que significa ~5-12 tokens/seg en modelos 7B. Eso esta bien para procesamiento por lotes y trabajo asincrono. Usa llamadas API para cubrir la brecha de velocidad en salidas orientadas al cliente.
{? endif ?}

> **Hablemos Claro:** Si tienes una RTX 3060 12GB, estas en mejor posicion que el 95% de los desarrolladores que intentan monetizar IA. Deja de esperar una 4090. La 3060 12GB es el Honda Civic de la IA local — confiable, eficiente, hace el trabajo. El dinero que gastarias en una mejora de GPU es mejor invertirlo en creditos API para calidad orientada al cliente mientras tus modelos locales se encargan del trabajo pesado.

#### Almacenamiento

```bash
# Linux/Mac
df -h

# Windows (PowerShell)
Get-PSDrive -PSProvider FileSystem | Select-Object Name, @{N='Used(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Free(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

**Lo que importa para los ingresos:**
- Los modelos LLM ocupan espacio: modelo 7B = ~4 GB, 13B = ~8 GB, 70B = ~40 GB (cuantizado).
- Necesitas espacio para datos de proyectos, bases de datos, caches y artefactos de salida.
- SSD es innegociable para cualquier cosa orientada al cliente. Cargar modelos desde HDD agrega 30-60 segundos de tiempo de inicio.
- Minimo practico: 500 GB SSD con al menos 100 GB libres.
- Comodo: 1 TB SSD. Manten los modelos en el SSD, archiva en HDD.
{? if profile.storage.free ?}
*Tienes {= profile.storage.free | fallback("?") =} libres en {= profile.storage.type | fallback("your drive") =}. {? if profile.storage.type == "SSD" ?}Bien — SSD significa carga rapida de modelos.{? elif profile.storage.type == "NVMe" ?}Excelente — NVMe es la opcion mas rapida para cargar modelos.{? else ?}Considera un SSD si aun no tienes uno — hace una diferencia real en los tiempos de carga de modelos.{? endif ?}*
{? endif ?}

#### Red

```bash
# Quick speed test (install speedtest-cli if needed)
# pip install speedtest-cli
speedtest-cli --simple

# Or just check your plan
# Upload speed matters more than download for serving
```

**Lo que importa para los ingresos:**
{? if profile.network.download ?}
*Tu conexion: {= profile.network.download | fallback("?") =} bajada / {= profile.network.upload | fallback("?") =} subida.*
{? endif ?}
- Velocidad de descarga: 50+ Mbps. Necesaria para descargar modelos, paquetes y datos.
- Velocidad de subida: Este es el cuello de botella que la mayoria ignora. Si estas sirviendo algo (APIs, resultados procesados, entregables), la subida importa.
  - 10 Mbps: Adecuada para entrega asincrona (archivos procesados, resultados por lotes).
  - 50+ Mbps: Requerida si ejecutas cualquier tipo de endpoint API local al que servicios externos acceden.
  - 100+ Mbps: Comodo para todo lo que cubre este curso.
- Latencia: Menos de 50ms hacia proveedores cloud principales. Ejecuta `ping api.openai.com` y `ping api.anthropic.com` para verificar.

#### Tiempo de Actividad

Esta es la especificacion en la que nadie piensa, pero separa a los aficionados de las personas que ganan dinero mientras duermen.

Preguntate:
- Puede tu equipo funcionar 24/7? (Energia, enfriamiento, ruido)
- Tienes un UPS para cortes de energia?
- Tu conexion a internet es lo suficientemente estable para flujos de trabajo automatizados?
- Puedes conectarte por SSH a tu maquina de forma remota si algo se rompe?

Si no puedes funcionar 24/7, esta bien — muchos flujos de ingresos en este curso son trabajos por lotes asincronos que ejecutas manualmente. Pero los que generan ingresos verdaderamente pasivos requieren tiempo de actividad.

{? if computed.os_family == "windows" ?}
**Configuracion rapida de tiempo de actividad (Windows):** Usa el Programador de Tareas para reinicio automatico, habilita Escritorio Remoto o instala Tailscale para acceso remoto, y configura tu BIOS para "restaurar al recuperar energia" para recuperarte de cortes.
{? endif ?}

**Configuracion rapida de tiempo de actividad (si la quieres):**

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

### Las Matematicas de la Electricidad

La gente ignora esto o lo exagera catastroficamente. Hagamos matematicas reales.

**Midiendo tu consumo real de energia:**

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

**Calculo del costo mensual:**

```
Costo mensual = (Watts / 1000) x Horas x Precio por kWh

Ejemplo: Desktop con RTX 3060, ejecutando inferencia 8 horas/dia, idle 16 horas/dia
- Inferencia: (250W / 1000) x 8h x 30 dias x $0.12/kWh = $7.20/mes
- Idle: (100W / 1000) x 16h x 30 dias x $0.12/kWh = $5.76/mes
- Total: ~$13/mes

Ejemplo: Mismo equipo, 24/7 inferencia
- (250W / 1000) x 24h x 30 dias x $0.12/kWh = $21.60/mes

Ejemplo: Mac Mini M2, 24/7
- (12W / 1000) x 24h x 30 dias x $0.12/kWh = $1.04/mes
```

{? if regional.country ?}
Tu tarifa de electricidad: aproximadamente {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh (basado en promedios de {= regional.country | fallback("your region") =}). Revisa tu factura real de servicios — las tarifas varian segun proveedor y hora del dia.
{? else ?}
El promedio de electricidad en EE.UU. es aproximadamente $0.12/kWh. Revisa tu tarifa real — varia enormemente. California puede ser $0.25/kWh. Algunos paises europeos llegan a $0.35/kWh. Partes del medio oeste de EE.UU. estan en $0.08/kWh.
{? endif ?}

**El punto:** Mantener tu equipo funcionando 24/7 para generar ingresos cuesta entre {= regional.currency_symbol | fallback("$") =}1-{= regional.currency_symbol | fallback("$") =}30/mes en electricidad. Si tus flujos de ingresos no pueden cubrir eso, el problema no es la electricidad — es el flujo de ingresos.

### Especificaciones Minimas por Tipo de Motor de Ingresos

Aqui tienes una vista previa de hacia donde nos dirigimos en el curso completo de STREETS. Por ahora, solo verifica donde se ubica tu equipo:

| Motor de Ingresos | CPU | RAM | GPU | Almacenamiento | Red |
|-------------------|-----|-----|-----|----------------|-----|
| **Automatizacion de contenido** (publicaciones de blog, newsletters) | 4+ nucleos | 16 GB | Opcional (respaldo API) | 50 GB libres | 10 Mbps subida |
| **Servicios de procesamiento de datos** | 8+ nucleos | 32 GB | Opcional | 200 GB libres | 50 Mbps subida |
| **Servicios de API de IA local** | 8+ nucleos | 32 GB | 8+ GB VRAM | 100 GB libres | 50 Mbps subida |
| **Herramientas de generacion de codigo** | 8+ nucleos | 16 GB | 8+ GB VRAM o API | 50 GB libres | 10 Mbps subida |
| **Procesamiento de documentos** | 4+ nucleos | 16 GB | Opcional | 100 GB libres | 10 Mbps subida |
| **Agentes autonomos** | 8+ nucleos | 32 GB | 12+ GB VRAM | 100 GB libres | 50 Mbps subida |

> **Error Comun:** "Necesito mejorar mi hardware antes de poder empezar." No. Empieza con lo que tienes. Usa llamadas API para cubrir las brechas que tu hardware no puede. Mejora cuando los ingresos lo justifiquen — no antes.

{@ insight engine_ranking @}

### Punto de Control de la Leccion 1

Ahora deberias tener anotado:
- [ ] Modelo de CPU, nucleos e hilos
- [ ] Cantidad de RAM
- [ ] Modelo de GPU y VRAM (o "ninguna")
- [ ] Almacenamiento disponible
- [ ] Velocidades de red (bajada/subida)
- [ ] Costo mensual estimado de electricidad para operacion 24/7
- [ ] Para que categorias de motores de ingresos califica tu equipo

Guarda estos numeros. Los ingresaras en tu Documento de Stack Soberano en la Leccion 6.

{? if computed.profile_completeness != "0" ?}
> **4DA ya recopilo la mayoria de estos numeros por ti.** Revisa los resumenes personalizados arriba — tu inventario de hardware esta parcialmente prellenado a partir de la deteccion del sistema.
{? endif ?}

*En el curso completo de STREETS, el Modulo R (Motores de Ingresos) te da guias especificas paso a paso para cada tipo de motor listado arriba — incluyendo el codigo exacto para construirlos y desplegarlos.*

---

## Leccion 2: El Stack de LLM Local

*"Configura Ollama para uso en produccion — no solo para chatear."*

### Por Que los LLMs Locales Importan para los Ingresos

Cada vez que llamas a la API de OpenAI, estas pagando alquiler. Cada vez que ejecutas un modelo localmente, esa inferencia es gratuita despues de la configuracion inicial. Las matematicas son simples:

- GPT-4o: ~$5 por millon de tokens de entrada, ~$15 por millon de tokens de salida
- Claude 3.5 Sonnet: ~$3 por millon de tokens de entrada, ~$15 por millon de tokens de salida
- Llama 3.1 8B local: $0 por millon de tokens (solo electricidad)

Si estas construyendo servicios que procesan miles de solicitudes, la diferencia entre $0 y $5-$15 por millon de tokens es la diferencia entre ganancia y punto de equilibrio.

Pero aqui esta el matiz que la mayoria se pierde: **los modelos locales y los de API sirven roles diferentes en un stack de ingresos.** Los modelos locales manejan el volumen. Los modelos API manejan la calidad critica, las salidas orientadas al cliente. Tu stack necesita ambos.

### Instalando Ollama

{? if settings.has_llm ?}
> **Ya tienes un LLM configurado:** {= settings.llm_provider | fallback("Local") =} / {= settings.llm_model | fallback("unknown model") =}. Si Ollama ya esta ejecutandose, salta a "Guia de Seleccion de Modelos" mas abajo.
{? endif ?}

Ollama es la base. Convierte tu maquina en un servidor de inferencia local con una API limpia.

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
> **Windows:** Usa el instalador de ollama.com o `winget install Ollama.Ollama`. Ollama se ejecuta como servicio en segundo plano automaticamente despues de la instalacion.
{? elif computed.os_family == "macos" ?}
> **macOS:** `brew install ollama` es la via mas rapida. Ollama aprovecha la memoria unificada de Apple Silicon — tus {= profile.ram.total | fallback("system") =} RAM se comparten entre cargas de trabajo de CPU y GPU.
{? elif computed.os_family == "linux" ?}
> **Linux:** El script de instalacion maneja todo. Si estas ejecutando {= profile.os.name | fallback("Linux") =}, Ollama se instala como un servicio systemd.
{? endif ?}

Verifica la instalacion:

```bash
ollama --version
# Should show version 0.5.x or higher (check https://ollama.com/download for latest)

# Start the server (if not auto-started)
ollama serve

# In another terminal, test it:
ollama run llama3.1:8b "Say hello in exactly 5 words"
```

> **Nota sobre versiones:** Ollama lanza actualizaciones frecuentemente. Los comandos de modelos y flags en este modulo fueron verificados contra Ollama v0.5.x (principios de 2026). Si estas leyendo esto despues, revisa [ollama.com/download](https://ollama.com/download) para la ultima version y [ollama.com/library](https://ollama.com/library) para los nombres de modelos actuales. Los conceptos fundamentales no cambian, pero las etiquetas especificas de modelos (por ejemplo, `llama3.1:8b`) pueden ser reemplazadas por versiones mas nuevas.

### Guia de Seleccion de Modelos

No descargues cada modelo que veas. Se estrategico. Esto es lo que debes descargar y cuando usar cada uno.

{? if computed.llm_tier ?}
> **Tu nivel de LLM (basado en hardware):** {= computed.llm_tier | fallback("unknown") =}. Las recomendaciones a continuacion estan etiquetadas para que puedas enfocarte en el nivel que coincide con tu equipo.
{? endif ?}

#### Nivel 1: El Caballo de Batalla (modelos 7B-8B)

```bash
# Pull your workhorse model
ollama pull llama3.1:8b
# Alternative: mistral (good for European languages)
ollama pull mistral:7b
```

**Usar para:**
- Clasificacion de texto ("Es este correo spam o legitimo?")
- Resumenes (condensar documentos largos en puntos clave)
- Extraccion de datos simple (extraer nombres, fechas, montos de texto)
- Analisis de sentimiento
- Etiquetado y categorizacion de contenido
- Generacion de embeddings (si usas un modelo con soporte de embeddings)

**Rendimiento (tipico):**
- RTX 3060 12GB: ~40-60 tokens/segundo
- RTX 4090: ~100-130 tokens/segundo
- M2 Pro 16GB: ~30-45 tokens/segundo
- Solo CPU (Ryzen 7 5800X): ~8-12 tokens/segundo

**Comparacion de costos:**
- 1 millon de tokens via GPT-4o-mini: ~$0.60
- 1 millon de tokens localmente (modelo 8B): ~$0.003 en electricidad
- Punto de equilibrio: ~5,000 tokens (ahorras dinero literalmente desde la primera solicitud)

#### Nivel 2: La Eleccion Equilibrada (modelos 13B-14B)

```bash
# Pull your balanced model
ollama pull llama3.1:14b
# Or for coding tasks:
ollama pull deepseek-coder-v2:16b
```

**Usar para:**
- Redaccion de contenido (publicaciones de blog, documentacion, textos de marketing)
- Generacion de codigo (funciones, scripts, boilerplate)
- Transformacion de datos complejos
- Tareas de razonamiento de multiples pasos
- Traduccion con matices

**Rendimiento (tipico):**
- RTX 3060 12GB: ~20-30 tokens/segundo (cuantizado)
- RTX 4090: ~60-80 tokens/segundo
- M2 Pro 32GB: ~20-30 tokens/segundo
- Solo CPU: ~3-6 tokens/segundo (no practico para tiempo real)

**Cuando usar en vez de 7B:** Cuando la calidad de salida del 7B no es suficiente pero no necesitas pagar por llamadas API. Prueba ambos en tu caso de uso real — a veces el 7B esta bien y solo estas desperdiciando computo.

{? if computed.gpu_tier == "capable" ?}
> **Territorio alcanzable del Nivel 3** — Tu {= profile.gpu.model | fallback("GPU") =} puede manejar 30B cuantizado con algo de esfuerzo, pero 70B esta fuera de alcance localmente. Considera llamadas API para tareas que necesiten calidad de nivel 70B.
{? endif ?}

#### Nivel 3: El Nivel de Calidad (modelos 30B-70B)

```bash
# Only pull these if you have the VRAM
# 30B needs ~20GB VRAM, 70B needs ~40GB VRAM (quantized)
ollama pull llama3.1:70b-instruct-q4_K_M
# Or the smaller but excellent:
ollama pull qwen2.5:32b
```

**Usar para:**
- Contenido orientado al cliente que necesita ser excelente
- Analisis y razonamiento complejos
- Generacion de contenido de formato largo
- Tareas donde la calidad impacta directamente si alguien te paga

**Rendimiento (tipico):**
- RTX 4090 (24GB): 70B a ~8-15 tokens/segundo (usable pero lento)
- GPU dual o 48GB+: 70B a ~20-30 tokens/segundo
- M3 Max 64GB: 70B a ~10-15 tokens/segundo

> **Hablemos Claro:** Si no tienes 24GB+ de VRAM, omite los modelos 70B por completo. Usa llamadas API para salidas de calidad critica. Un modelo 70B ejecutandose a 3 tokens/segundo desde la RAM del sistema es tecnicamente posible pero practicamente inutil para cualquier flujo de trabajo generador de ingresos. Tu tiempo tiene valor.

#### Nivel 4: Modelos API (Cuando lo Local No Alcanza)

Los modelos locales son para volumen y privacidad. Los modelos API son para techos de calidad y capacidades especializadas.

**Cuando usar modelos API:**
- Salida orientada al cliente donde calidad = ingresos (textos de venta, contenido premium)
- Cadenas de razonamiento complejas que los modelos mas pequenos fallan
- Tareas de vision/multimodales (analizar imagenes, capturas de pantalla, documentos)
- Cuando necesitas salida JSON estructurada con alta fiabilidad
- Cuando la velocidad importa y tu hardware local es lento

**Tabla de comparacion de costos (a principios de 2025 — revisa precios actuales):**

| Modelo | Entrada (por 1M tokens) | Salida (por 1M tokens) | Mejor Para |
|--------|------------------------|------------------------|------------|
| GPT-4o-mini | $0.15 | $0.60 | Trabajo de volumen economico (cuando lo local no esta disponible) |
| GPT-4o | $2.50 | $10.00 | Vision, razonamiento complejo |
| Claude 3.5 Sonnet | $3.00 | $15.00 | Codigo, analisis, contexto largo |
| Claude 3.5 Haiku | $0.80 | $4.00 | Rapido, economico, buen balance de calidad |
| DeepSeek V3 | $0.27 | $1.10 | Economico, rendimiento solido |

**La estrategia hibrida:**
1. El LLM local 7B/13B maneja el 80% de las solicitudes (clasificacion, extraccion, resumen)
2. La API maneja el 20% de las solicitudes (generacion de calidad critica, tareas complejas)
3. Tu costo efectivo: ~$0.50-2.00 por millon de tokens combinados (en vez de $5-15 con API pura)

Este enfoque hibrido es como construyes servicios con margenes saludables. Mas sobre esto en el Modulo R.

### Configuracion de Produccion

Ejecutar Ollama para trabajo de ingresos es diferente de ejecutarlo para chat personal. Asi es como configurarlo correctamente.

{? if computed.has_nvidia ?}
> **GPU NVIDIA detectada ({= profile.gpu.model | fallback("unknown") =}).** Ollama usara automaticamente aceleracion CUDA. Asegurate de que tus drivers NVIDIA esten actualizados — ejecuta `nvidia-smi` para verificar. Para mejor rendimiento con {= profile.gpu.vram | fallback("your") =} VRAM, la configuracion `OLLAMA_MAX_LOADED_MODELS` a continuacion debe coincidir con cuantos modelos caben en tu VRAM simultaneamente.
{? endif ?}

#### Establecer Variables de Entorno

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

#### Crear un Modelfile para Tu Carga de Trabajo

En vez de usar la configuracion predeterminada del modelo, crea un Modelfile personalizado ajustado para tu carga de trabajo de ingresos:

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

#### Procesamiento por Lotes y Gestion de Cola

Para cargas de trabajo de ingresos, a menudo necesitaras procesar muchos elementos. Aqui tienes una configuracion basica de procesamiento por lotes:

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

### Haciendo Benchmark a TU Equipo

No confies en los benchmarks de otros. Mide los tuyos:

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

Anota tus tokens/segundo para cada modelo. Este numero determina que flujos de trabajo de ingresos son practicos para tu equipo.

{@ insight stack_fit @}

**Requisitos de velocidad por caso de uso:**
- Procesamiento por lotes (asincrono): 5+ tokens/seg esta bien (no te importa la latencia)
- Herramientas interactivas (el usuario espera): 20+ tokens/seg minimo
- API en tiempo real (orientada al cliente): 30+ tokens/seg para buena experiencia de usuario
- Chat en streaming: 15+ tokens/seg se siente responsivo

### Asegurando Tu Servidor de Inferencia Local

{? if computed.os_family == "windows" ?}
> **Nota para Windows:** Ollama en Windows se enlaza a localhost por defecto. Verifica con `netstat -an | findstr 11434` en PowerShell. Usa el Firewall de Windows para bloquear acceso externo al puerto 11434.
{? elif computed.os_family == "macos" ?}
> **Nota para macOS:** Ollama en macOS se enlaza a localhost por defecto. Verifica con `lsof -i :11434`. El firewall de macOS deberia bloquear conexiones externas automaticamente.
{? endif ?}

Tu instancia de Ollama nunca deberia ser accesible desde internet a menos que lo pretendas explicitamente.

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

> **Error Comun:** Enlazar Ollama a 0.0.0.0 por "conveniencia" y olvidarse de ello. Cualquiera que encuentre tu IP puede usar tu GPU para inferencia gratuita. Peor aun, pueden extraer pesos del modelo y prompts del sistema. Siempre localhost. Siempre tunel.

### Punto de Control de la Leccion 2

Ahora deberias tener:
- [ ] Ollama instalado y funcionando
- [ ] Al menos un modelo caballo de batalla descargado (llama3.1:8b o equivalente)
- [ ] Un Modelfile personalizado para tu carga de trabajo esperada
- [ ] Numeros de benchmark: tokens/segundo para cada modelo en tu equipo
- [ ] Ollama enlazado solo a localhost

*En el curso completo de STREETS, el Modulo T (Fosos Tecnicos) te muestra como construir configuraciones de modelos propietarias, pipelines ajustados y cadenas de herramientas personalizadas que los competidores no pueden replicar facilmente. El Modulo R (Motores de Ingresos) te da los servicios exactos para construir sobre este stack.*

---

## Leccion 3: La Ventaja de Privacidad

*"Tu configuracion privada ES una ventaja competitiva — no solo una preferencia."*

### La Privacidad Es una Caracteristica del Producto, No una Limitacion

La mayoria de los desarrolladores configuran infraestructura local porque personalmente valoran la privacidad, o porque disfrutan experimentando. Eso esta bien. Pero estas dejando dinero sobre la mesa si no te das cuenta de que **la privacidad es una de las caracteristicas mas comercializables en tecnologia ahora mismo.**

Esto es por que: cada vez que una empresa envia datos a la API de OpenAI, esos datos pasan por un tercero. Para muchas empresas — especialmente las de salud, finanzas, legal, gobierno y empresas con base en la UE — esto es un problema real. No uno teorico. Un problema de "no podemos usar esta herramienta porque cumplimiento dijo que no."

Tu, ejecutando modelos localmente en tu maquina, no tienes ese problema.

### El Viento Regulatorio a Favor

El entorno regulatorio se mueve en tu direccion. Rapido.

{? if regional.country == "US" ?}
> **Basado en EE.UU.:** Las regulaciones a continuacion que mas te importan son HIPAA, SOC 2, ITAR y las leyes de privacidad a nivel estatal (California CCPA, etc.). Las regulaciones de la UE aun importan — afectan tu capacidad de servir a clientes europeos, que es un mercado lucrativo.
{? elif regional.country == "GB" ?}
> **Basado en el Reino Unido:** Post-Brexit, el Reino Unido tiene su propio marco de proteccion de datos (UK GDPR + Data Protection Act 2018). Tu ventaja de procesamiento local es especialmente fuerte para servir a servicios financieros del Reino Unido y trabajo adyacente al NHS.
{? elif regional.country == "DE" ?}
> **Basado en Alemania:** Estas en uno de los entornos de proteccion de datos mas estrictos del mundo. Esto es una *ventaja* — los clientes alemanes ya entienden por que importa el procesamiento local, y pagaran por ello.
{? elif regional.country == "AU" ?}
> **Basado en Australia:** La Privacy Act 1988 y los Australian Privacy Principles (APPs) rigen tus obligaciones. El procesamiento local es un fuerte argumento de venta para clientes gubernamentales y de salud bajo la My Health Records Act.
{? endif ?}

**EU AI Act (vigente desde 2024-2026):**
- Los sistemas de IA de alto riesgo necesitan pipelines de procesamiento de datos documentados
- Las empresas deben demostrar donde fluyen los datos y quien los procesa
- El procesamiento local simplifica dramaticamente el cumplimiento
- Las empresas de la UE estan buscando activamente proveedores de servicios de IA que puedan garantizar residencia de datos en la UE

**GDPR (ya vigente):**
- "Procesamiento de datos" incluye enviar texto a una API de LLM
- Las empresas necesitan Acuerdos de Procesamiento de Datos con cada tercero
- El procesamiento local elimina al tercero por completo
- Este es un argumento de venta real: "Tus datos nunca salen de tu infraestructura. No hay DPA de terceros que negociar."

**Regulaciones especificas por industria:**
- **HIPAA (Salud en EE.UU.):** Los datos de pacientes no pueden enviarse a APIs de IA de consumo sin un BAA (Business Associate Agreement). La mayoria de los proveedores de IA no ofrecen BAAs para acceso API. El procesamiento local evita esto por completo.
- **SOC 2 (Empresa):** Las empresas que pasan auditorias SOC 2 necesitan documentar cada procesador de datos. Menos procesadores = auditorias mas faciles.
- **ITAR (Defensa EE.UU.):** Los datos tecnicos controlados no pueden salir de la jurisdiccion de EE.UU. Los proveedores de IA cloud con infraestructura internacional son problematicos.
- **PCI DSS (Finanzas):** El procesamiento de datos de tarjetahabientes tiene requisitos estrictos sobre por donde viajan los datos.

### Como Posicionar la Privacidad en Conversaciones de Venta

No necesitas ser un experto en cumplimiento. Necesitas entender tres frases y saber cuando usarlas:

**Frase 1: "Tus datos nunca salen de tu infraestructura."**
Usar cuando: Hablas con cualquier prospecto consciente de la privacidad. Este es el gancho universal.

**Frase 2: "No se requiere acuerdo de procesamiento de datos con terceros."**
Usar cuando: Hablas con empresas europeas o cualquier empresa con equipo legal/de cumplimiento. Esto les ahorra semanas de revision legal.

**Frase 3: "Auditoria completa, procesamiento de inquilino unico."**
Usar cuando: Hablas con empresas o industrias reguladas. Necesitan demostrar su pipeline de IA ante auditores.

**Ejemplo de posicionamiento (para tu pagina de servicio o propuestas):**

> "A diferencia de los servicios de IA basados en la nube, [Tu Servicio] procesa todos los datos localmente en hardware dedicado. Tus documentos, codigo y datos nunca salen del entorno de procesamiento. No hay APIs de terceros en el pipeline, no hay acuerdos de intercambio de datos que negociar, y hay registro completo de auditoria de cada operacion. Esto hace que [Tu Servicio] sea adecuado para organizaciones con requisitos estrictos de manejo de datos, incluyendo entornos de cumplimiento GDPR, HIPAA y SOC 2."

Ese parrafo, en una landing page, atraera exactamente a los clientes que pagaran tarifas premium.

### La Justificacion de Precios Premium

Aqui esta el caso de negocio en numeros concretos:

**Servicio estandar de procesamiento de IA (usando APIs en la nube):**
- Los datos del cliente van a OpenAI/Anthropic/Google
- Estas compitiendo con cada desarrollador que puede llamar una API
- Tarifa de mercado: $0.01-0.05 por documento procesado
- Basicamente estas revendiendo acceso API con un margen

**Servicio de procesamiento de IA con privacidad primero (tu stack local):**
- Los datos del cliente se quedan en tu maquina
- Estas compitiendo con un grupo mucho mas pequeno de proveedores
- Tarifa de mercado: $0.10-0.50 por documento procesado (premium de 5-10x)
- Estas vendiendo infraestructura + experiencia + cumplimiento

El premium de privacidad es real: **5x a 10x** sobre servicios commoditizados basados en la nube para la misma tarea subyacente. Y los clientes que lo pagan son mas leales, menos sensibles al precio y tienen presupuestos mas grandes.

{@ insight competitive_position @}

### Configurando Espacios de Trabajo Aislados

Si tienes un empleo de dia (la mayoria de ustedes si), necesitas separacion limpia entre el trabajo del empleador y el trabajo de ingresos. Esto no es solo proteccion legal — es higiene operativa.

{? if computed.os_family == "windows" ?}
> **Consejo para Windows:** Crea una cuenta de usuario de Windows separada para trabajo de ingresos (Configuracion > Cuentas > Familia y otros usuarios > Agregar a alguien mas). Esto te da un entorno completamente aislado — perfiles de navegador separados, rutas de archivos separadas, variables de entorno separadas. Cambia entre cuentas con Win+L.
{? endif ?}

**Opcion 1: Cuentas de usuario separadas (recomendado)**

```bash
# Linux: Create a dedicated user for income work
sudo useradd -m -s /bin/bash income
sudo passwd income

# Switch to income user for all revenue work
su - income

# All income projects, API keys, and data live under /home/income/
```

**Opcion 2: Espacios de trabajo en contenedores**

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

**Opcion 3: Maquina fisica separada (la mas infalible)**

Si esto va en serio y tus ingresos lo justifican, una maquina dedicada elimina todas las preguntas. Un Dell OptiPlex usado con una RTX 3060 cuesta $400-600 y se paga solo en el primer mes de trabajo con clientes.

**Lista de verificacion de separacion minima:**
- [ ] Proyectos de ingresos en un directorio separado (nunca mezclados con repos del empleador)
- [ ] Claves API separadas para trabajo de ingresos (nunca uses claves proporcionadas por el empleador)
- [ ] Perfil de navegador separado para cuentas relacionadas con ingresos
- [ ] El trabajo de ingresos nunca se hace en hardware del empleador
- [ ] El trabajo de ingresos nunca se hace en la red del empleador (usa tu internet personal o una VPN)
- [ ] Cuenta de GitHub/GitLab separada para proyectos de ingresos (opcional pero limpio)

> **Error Comun:** Usar la clave API de OpenAI de tu empleador "solo para probar" tu proyecto paralelo. Esto crea un rastro de papel que el dashboard de facturacion de tu empleador puede ver, y enturbia las aguas de la propiedad intelectual. Consigue tus propias claves. Son baratas.

### Punto de Control de la Leccion 3

Ahora deberias entender:
- [ ] Por que la privacidad es una caracteristica de producto comercializable, no solo una preferencia personal
- [ ] Que regulaciones crean demanda para procesamiento de IA local
- [ ] Tres frases para usar en conversaciones de venta sobre privacidad
- [ ] Como los servicios con privacidad primero cobran un premium de 5-10x
- [ ] Como separar el trabajo de ingresos del trabajo del empleador

*En el curso completo de STREETS, el Modulo E (Ventaja en Evolucion) te ensena como rastrear cambios regulatorios y posicionarte antes de los nuevos requisitos de cumplimiento antes de que tus competidores siquiera sepan que existen.*

---

## Leccion 4: El Minimo Legal

*"Quince minutos de configuracion legal ahora previenen meses de problemas despues."*

### Esto No Es Asesoramiento Legal

Soy desarrollador, no abogado. Lo que sigue es una lista practica que la mayoria de los desarrolladores en la mayoria de las situaciones deberian abordar. Si tu situacion es compleja (participacion accionaria en tu empleador, acuerdo de no competencia con terminos especificos, etc.), gasta $200 en una consulta de 30 minutos con un abogado laboral. Es el mejor retorno de inversion que obtendras.

### Paso 1: Lee Tu Contrato de Trabajo

Encuentra tu contrato de trabajo o carta de oferta. Busca estas secciones:

**Clausula de Asignacion de Propiedad Intelectual** — Busca lenguaje como:
- "Todas las invenciones, desarrollos y productos de trabajo..."
- "...creados durante el termino del empleo..."
- "...relacionados con el negocio o negocio anticipado de la Empresa..."

**Frases clave que te restringen:**
- "Todo producto de trabajo creado durante el empleo pertenece a la Empresa" (amplio — potencialmente problematico)
- "Producto de trabajo creado usando recursos de la Empresa" (mas acotado — usualmente esta bien si usas tu propio equipo)
- "Relacionado con el negocio actual o anticipado de la Empresa" (depende de lo que haga tu empleador)

**Frases clave que te liberan:**
- "Excluyendo trabajo realizado enteramente en el tiempo propio del Empleado con los recursos propios del Empleado y no relacionado con el negocio de la Empresa" (esta es tu excepcion — muchos estados de EE.UU. la requieren)
- Algunos estados (California, Washington, Minnesota, Illinois, otros) tienen leyes que limitan las reclamaciones de PI del empleador sobre proyectos personales, independientemente de lo que diga el contrato.

### La Prueba de las 3 Preguntas

Para cualquier proyecto de ingresos, pregunta:

1. **Tiempo:** Estas haciendo este trabajo en tu propio tiempo? (No durante horas de trabajo, no durante turnos de guardia)
2. **Equipo:** Estas usando tu propio hardware, tu propio internet, tus propias claves API? (No la laptop del empleador, no la VPN del empleador, no las cuentas cloud del empleador)
3. **Materia:** Esto no esta relacionado con el negocio de tu empleador? (Si trabajas en una empresa de IA para salud y quieres vender servicios de IA para salud... eso es un problema. Si trabajas en una empresa de IA para salud y quieres vender procesamiento de documentos para agentes inmobiliarios... eso esta bien.)

Si las tres respuestas estan limpias, es casi seguro que estas bien. Si alguna respuesta es turbia, obtiene claridad antes de proceder.

> **Hablemos Claro:** La gran mayoria de los desarrolladores que hacen trabajo paralelo nunca tienen un problema. A los empleadores les importa proteger ventajas competitivas, no evitar que ganes dinero extra en proyectos no relacionados. Pero "es casi seguro que esta bien" no es "definitivamente esta bien." Si tu contrato es inusualmente amplio, ten una conversacion con tu gerente o recursos humanos — o consulta a un abogado. La desventaja de no verificar es mucho peor que la leve incomodidad de preguntar.

### Paso 2: Elige una Estructura de Negocio

Necesitas una entidad legal para separar tus activos personales de tus actividades comerciales, y para abrir la puerta a banca empresarial, procesamiento de pagos y beneficios fiscales.

{? if regional.country ?}
> **Tu ubicacion: {= regional.country | fallback("Unknown") =}.** El tipo de entidad recomendado para tu region es una **{= regional.business_entity_type | fallback("LLC or equivalent") =}**, con costos tipicos de registro de {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Desplazate a la seccion de tu pais abajo, o lee todas las secciones para entender como operan los clientes en otras regiones.
{? endif ?}

{? if regional.country == "US" ?}
#### Estados Unidos (Tu Region)
{? else ?}
#### Estados Unidos
{? endif ?}

| Estructura | Costo | Proteccion | Mejor Para |
|------------|-------|------------|------------|
| **Empresa Individual** (predeterminada) | $0 | Ninguna (responsabilidad personal) | Probar el terreno. Primeros $1K. |
| **LLC de Un Solo Miembro** | $50-500 (varia por estado) | Proteccion de activos personales | Trabajo activo de ingresos. La mayoria de los desarrolladores deberian empezar aqui. |
| **Eleccion S-Corp** (sobre una LLC) | Costo de LLC + $0 por la eleccion | Igual que LLC + beneficios de impuestos de nomina | Cuando ganas consistentemente $40K+/ano de esto |

**Recomendado para desarrolladores en EE.UU.:** LLC de Un Solo Miembro en tu estado de residencia.

**Estados mas baratos para constituirse:** Wyoming ($100, sin impuesto estatal sobre la renta), New Mexico ($50), Montana ($70). Pero constituirse en tu estado de residencia es usualmente lo mas simple a menos que tengas una razon especifica para no hacerlo.

**Como registrarse:**
1. Ve al sitio web del Secretary of State de tu estado
2. Busca "form LLC" o "business entity filing"
3. Presenta los Articles of Organization (formulario de 10 minutos)
4. Obtiene un EIN del IRS (gratis, toma 5 minutos en irs.gov)

{? if regional.country == "GB" ?}
#### Reino Unido (Tu Region)
{? else ?}
#### Reino Unido
{? endif ?}

| Estructura | Costo | Proteccion | Mejor Para |
|------------|-------|------------|------------|
| **Sole Trader** | Gratis (registrate con HMRC) | Ninguna | Primeros ingresos. Probando. |
| **Limited Company (Ltd)** | ~$15 via Companies House | Proteccion de activos personales | Cualquier trabajo serio de ingresos. |

**Recomendado:** Ltd company via Companies House. Toma unos 20 minutos y cuesta GBP 12.

#### Union Europea

Varia significativamente por pais, pero el patron general:

- **Alemania:** Einzelunternehmer (empresa individual) para empezar, GmbH para trabajo serio (pero GmbH requiere EUR 25,000 de capital — considera UG por EUR 1)
- **Paises Bajos:** Eenmanszaak (empresa individual, registro gratuito) o BV (comparable a Ltd)
- **Francia:** Micro-entrepreneur (simplificado, recomendado para empezar)
- **Estonia:** e-Residency + OUE (popular para no residentes, completamente en linea)

{? if regional.country == "AU" ?}
#### Australia (Tu Region)
{? else ?}
#### Australia
{? endif ?}

| Estructura | Costo | Proteccion | Mejor Para |
|------------|-------|------------|------------|
| **Sole Trader** | ABN gratuito | Ninguna | Empezando |
| **Pty Ltd** | ~AUD 500-800 via ASIC | Proteccion de activos personales | Ingresos serios |

**Recomendado:** Comienza con un ABN de Sole Trader (gratis, instantaneo), pasa a Pty Ltd cuando estes ganando consistentemente.

### Paso 3: Procesamiento de Pagos (configuracion de 15 minutos)

Necesitas una forma de recibir pagos. Configura esto ahora, no cuando tu primer cliente este esperando.

{? if regional.payment_processors ?}
> **Recomendado para {= regional.country | fallback("your region") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy") =}
{? endif ?}

**Stripe (recomendado para la mayoria de los desarrolladores):**

```
1. Ve a stripe.com
2. Crea una cuenta con tu correo de negocio
3. Completa la verificacion de identidad
4. Conecta tu cuenta bancaria de negocio
5. Ya puedes aceptar pagos, crear facturas y configurar suscripciones
```

Tiempo: ~15 minutos. Puedes empezar a aceptar pagos inmediatamente (Stripe retiene fondos por 7 dias en cuentas nuevas).

**Lemon Squeezy (recomendado para productos digitales):**

Si vendes productos digitales (plantillas, herramientas, cursos, SaaS), Lemon Squeezy actua como tu Merchant of Record. Esto significa:
- Manejan impuestos de venta, IVA y GST por ti a nivel global
- No necesitas registrarte para IVA en la UE
- Manejan reembolsos y disputas

```
1. Ve a lemonsqueezy.com
2. Crea una cuenta
3. Configura tu tienda
4. Agrega productos
5. Ellos manejan todo lo demas
```

**Stripe Atlas (para desarrolladores internacionales o quienes quieren una entidad en EE.UU.):**

Si estas fuera de EE.UU. pero quieres vender a clientes estadounidenses con una entidad de EE.UU.:
- $500 de tarifa unica
- Crea una LLC en Delaware por ti
- Configura una cuenta bancaria en EE.UU. (via Mercury o Stripe)
- Proporciona servicio de agente registrado
- Toma aproximadamente 1-2 semanas

### Paso 4: Politica de Privacidad y Terminos de Servicio

Si vendes cualquier servicio o producto en linea, necesitas estos. No pagues a un abogado por plantillas estandar.

**Fuentes gratuitas y confiables para plantillas:**
- **Termly.io** — Generador gratuito de politica de privacidad y ToS. Responde preguntas, obtiene documentos.
- **Avodocs.com** — Documentos legales de codigo abierto para startups. Gratis.
- **choosealicense.com de GitHub** — Para licencias de proyectos de codigo abierto especificamente.
- **Politicas de codigo abierto de Basecamp** — Busca "Basecamp open source policies" — buenas plantillas en lenguaje claro.

**Lo que tu politica de privacidad debe cubrir (si procesas datos de clientes):**
- Que datos recopilas
- Como los procesas (localmente — esta es tu ventaja)
- Por cuanto tiempo los retienes
- Como los clientes pueden solicitar eliminacion
- Si algun tercero accede a los datos (idealmente: ninguno)

**Tiempo:** 30 minutos con un generador de plantillas. Listo.

### Paso 5: Cuenta Bancaria Separada

No manejes ingresos de negocio a traves de tu cuenta personal. Las razones:

1. **Claridad fiscal:** Cuando llega la temporada de impuestos, necesitas saber exactamente que fue ingreso de negocio y que no.
2. **Proteccion legal:** Si tienes una LLC, mezclar fondos personales y de negocio puede "perforar el velo corporativo" — lo que significa que un tribunal puede ignorar la proteccion de responsabilidad de tu LLC.
3. **Profesionalismo:** Facturas de "Consultoria de Juan LLC" llegando a una cuenta de negocio dedicada se ve legitimo. Pagos a tu Venmo personal no.

**Banca empresarial gratuita o de bajo costo:**
{? if regional.country == "US" ?}
- **Mercury** (recomendado para ti) — Gratis, disenado para startups. Excelente API si quieres automatizar contabilidad despues.
- **Relay** — Gratis, bueno para separar flujos de ingresos en subcuentas.
{? elif regional.country == "GB" ?}
- **Starling Bank** (recomendado para ti) — Cuenta de negocio gratuita, configuracion instantanea.
- **Wise Business** — Multi-moneda de bajo costo. Genial si sirves a clientes internacionales.
{? else ?}
- **Mercury** (EE.UU.) — Gratis, disenado para startups. Excelente API si quieres automatizar contabilidad despues.
- **Relay** (EE.UU.) — Gratis, bueno para separar flujos de ingresos en subcuentas.
- **Starling Bank** (Reino Unido) — Cuenta de negocio gratuita.
{? endif ?}
- **Wise Business** (Internacional) — Multi-moneda de bajo costo. Genial para recibir pagos en USD, EUR, GBP, etc.
- **Qonto** (UE) — Banca empresarial limpia para empresas europeas.

Abre la cuenta ahora. Toma 10-15 minutos en linea y 1-3 dias para verificacion.

### Paso 6: Conceptos Basicos de Impuestos para Ingresos Paralelos de Desarrolladores

{? if regional.tax_note ?}
> **Nota fiscal para {= regional.country | fallback("your region") =}:** {= regional.tax_note | fallback("Consult a local tax professional for specifics.") =}
{? endif ?}

> **Hablemos Claro:** Los impuestos son lo que la mayoria de los desarrolladores ignoran hasta abril, y luego entran en panico. Dedicar 30 minutos ahora te ahorra dinero y estres reales.

**Estados Unidos:**
- Ingresos paralelos superiores a $400/ano requieren impuesto de autoempleo (~15.3% para Seguro Social + Medicare)
- Mas la tasa de impuesto sobre la renta regular sobre la ganancia neta
- **Impuestos estimados trimestrales:** Si debes mas de $1,000 en impuestos, el IRS espera pagos trimestrales (15 de abril, 15 de junio, 15 de sept, 15 de enero). El pago insuficiente genera penalidades.
- Reserva **25-30%** del ingreso neto para impuestos. Ponlo en una cuenta de ahorro separada inmediatamente.

**Deducciones comunes para ingresos paralelos de desarrolladores:**
- Costos de API (OpenAI, Anthropic, etc.) — 100% deducibles
- Compras de hardware usadas para el negocio — depreciables o deduccion Seccion 179
- Costo de electricidad atribuible al uso comercial
- Suscripciones de software usadas para trabajo de ingresos
- Deduccion de oficina en casa (simplificada: $5/pie cuadrado, hasta 300 pies cuadrados = $1,500)
- Internet (porcentaje de uso comercial)
- Nombres de dominio, hosting, servicios de email
- Desarrollo profesional (cursos, libros) relacionados con tu trabajo de ingresos

**Reino Unido:**
- Reporta via declaracion de impuestos Self Assessment
- Ingresos comerciales bajo GBP 1,000: libres de impuestos (Trading Allowance)
- Por encima de eso: paga Income Tax + Class 4 NICs sobre ganancias
- Fechas de pago: 31 de enero y 31 de julio

**Registra todo desde el dia uno.** Usa una hoja de calculo simple si no tienes nada mas:

```
| Fecha      | Categoria   | Descripcion          | Monto   | Tipo    |
|------------|-------------|----------------------|---------|---------|
| 2025-01-15 | API         | Credito Anthropic    | -$20.00 | Gasto   |
| 2025-01-18 | Ingresos    | Factura cliente #001 | +$500.00| Ingreso |
| 2025-01-20 | Software    | Plan Vercel Pro      | -$20.00 | Gasto   |
| 2025-01-20 | Reserva Imp | 30% de ingreso neto  | -$138.00| Transfer|
```

> **Error Comun:** "Me encargo de los impuestos despues." Despues es el Q4, debes $3,000 en impuestos estimados mas penalidades, y ya gastaste el dinero. Automatiza: cada vez que un ingreso llega a tu cuenta de negocio, transfiere el 30% a una cuenta de ahorro para impuestos inmediatamente.

### Punto de Control de la Leccion 4

Ahora deberias tener (o tener un plan para):
- [ ] Leida la clausula de PI de tu contrato de trabajo
- [ ] Aprobada la Prueba de las 3 Preguntas para tu trabajo de ingresos planeado
- [ ] Elegida una estructura de negocio (o decidido empezar como empresa individual)
- [ ] Procesamiento de pagos configurado (Stripe o Lemon Squeezy)
- [ ] Politica de privacidad y ToS de un generador de plantillas
- [ ] Cuenta bancaria de negocio separada (o solicitud enviada)
- [ ] Estrategia fiscal: reserva del 30% + calendario de pagos trimestrales

*En el curso completo de STREETS, el Modulo E (Manual de Ejecucion) incluye plantillas de modelado financiero que calculan automaticamente tus obligaciones fiscales, rentabilidad del proyecto y puntos de equilibrio para cada motor de ingresos.*

---

## Leccion 5: El Presupuesto de {= regional.currency_symbol | fallback("$") =}200/mes

*"Tu negocio tiene una tasa de quema. Conocela. Controlala. Haz que produzca."*

### Por Que {= regional.currency_symbol | fallback("$") =}200/mes

Doscientos {= regional.currency | fallback("dollars") =} por mes es el presupuesto minimo viable para una operacion de ingresos de desarrollador. Es suficiente para ejecutar servicios reales, servir a clientes reales y generar ingresos reales. Tambien es lo suficientemente pequeno como para que si nada funciona, no hayas apostado todo.

El objetivo es simple: **convertir {= regional.currency_symbol | fallback("$") =}200/mes en {= regional.currency_symbol | fallback("$") =}600+/mes dentro de 90 dias.** Si puedes hacer eso, tienes un negocio. Si no puedes, cambias de estrategia — no aumentas el presupuesto.

### El Desglose del Presupuesto

#### Nivel 1: Creditos API — $50-100/mes

Este es tu computo de produccion para calidad orientada al cliente.

**Asignacion inicial recomendada:**

```
Anthropic (Claude):     $40/mes  — Tu principal para calidad de salida
OpenAI (GPT-4o-mini):   $20/mes  — Trabajo de volumen economico, respaldo
DeepSeek:               $10/mes  — Tareas de presupuesto, experimentacion
Reserva:                $30/mes  — Desborde o prueba de nuevos proveedores
```

**Como gestionar el gasto en API:**

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

**La estrategia de gasto hibrida:**
- Usa LLMs locales para el 80% del procesamiento (clasificacion, extraccion, resumen, borradores)
- Usa llamadas API para el 20% del procesamiento (pase final de calidad, razonamiento complejo, salida orientada al cliente)
- Tu costo efectivo por tarea baja dramaticamente vs. uso exclusivo de API

{? if computed.monthly_electricity_estimate ?}
> **Tu costo estimado de electricidad:** {= regional.currency_symbol | fallback("$") =}{= computed.monthly_electricity_estimate | fallback("13") =}/mes para operacion 24/7 a {= regional.currency_symbol | fallback("$") =}{= regional.electricity_kwh | fallback("0.12") =}/kWh. Esto ya esta incluido en tu costo operativo efectivo.
{? endif ?}

#### Nivel 2: Infraestructura — {= regional.currency_symbol | fallback("$") =}30-50/mes

```
Nombre de dominio:      $12/ano ($1/mes)     — Namecheap, Cloudflare, Porkbun
Email (negocio):        $0-6/mes             — Zoho Mail gratis, o Google Workspace $6
VPS (opcional):         $5-20/mes            — Para hospedar servicios ligeros
                                               Hetzner ($4), DigitalOcean ($6), Railway ($5)
DNS/CDN:                $0/mes               — Cloudflare tier gratuito
Hosting (estatico):     $0/mes               — Vercel, Netlify, Cloudflare Pages (tiers gratuitos)
```

**Necesitas un VPS?**

Si tu modelo de ingresos es:
- **Vender productos digitales:** No. Hospeda en Vercel/Netlify gratis. Usa Lemon Squeezy para la entrega.
- **Ejecutar procesamiento asincrono para clientes:** Tal vez. Puedes ejecutar trabajos en tu equipo local y entregar resultados. Un VPS agrega fiabilidad.
- **Ofrecer un servicio API:** Si, probablemente. Un VPS de $5-10 actua como un gateway API ligero, incluso si el procesamiento pesado ocurre en tu maquina local.
- **Vender SaaS:** Si. Pero empieza con el tier mas barato y escala.

**Infraestructura inicial recomendada:**

```
Equipo local — computo principal, inferencia LLM, procesamiento pesado
   |
   +-- Tunel SSH o VPN WireGuard
   |
VPS de $5 (Hetzner/DigitalOcean) — gateway API, receptor de webhooks, hosting estatico
   |
   +-- Cloudflare (gratis) — DNS, CDN, proteccion DDoS
   |
Vercel/Netlify (gratis) — sitio de marketing, landing pages, documentacion
```

Costo total de infraestructura: $5-20/mes. El resto son tiers gratuitos.

#### Nivel 3: Herramientas — {= regional.currency_symbol | fallback("$") =}20-30/mes

```
Analiticas:             $0/mes    — Plausible Cloud ($9) o autoalojado,
                                    o Vercel Analytics (tier gratuito)
                                    o simplemente analiticas de Cloudflare (gratis)
Email marketing:        $0/mes    — Buttondown (gratis hasta 100 suscriptores),
                                    Resend ($0 por 3K emails/mes)
Monitoreo:              $0/mes    — UptimeRobot (gratis, 50 monitores),
                                    Better Stack (tier gratuito)
Diseno:                 $0/mes    — Figma (gratis), Canva (tier gratuito)
Contabilidad:           $0/mes    — Wave (gratis), o una hoja de calculo
                                    Hledger (gratis, contabilidad en texto plano)
```

> **Hablemos Claro:** Puedes ejecutar todo tu stack de herramientas en tiers gratuitos al empezar. Los $20-30 asignados aqui son para cuando superes los tiers gratuitos o quieras una funcion premium especifica. No los gastes solo porque estan en el presupuesto. Presupuesto no gastado es ganancia.

#### Nivel 4: Reserva — {= regional.currency_symbol | fallback("$") =}0-30/mes

Este es tu fondo de "cosas que no anticipe":
- Un pico de costos API por un trabajo por lotes inesperadamente grande
- Una herramienta que necesitas para un proyecto especifico de un cliente
- Compra de emergencia de un dominio cuando encuentras el nombre perfecto
- Una compra unica (tema, plantilla, set de iconos)

Si no usas la reserva, se acumula. Despues de 3 meses de reserva sin usar, considera reasignar a creditos API o infraestructura.

### El Calculo de ROI

Este es el unico numero que importa:

```
Ingresos Mensuales - Costos Mensuales = Ganancia Neta
Ganancia Neta / Costos Mensuales = Multiplo de ROI

Ejemplo:
$600 ingresos - $200 costos = $400 ganancia
$400 / $200 = 2x ROI

El objetivo: 3x ROI ($600+ ingresos con $200 de gasto)
El minimo: 1x ROI ($200 ingresos = punto de equilibrio)
Por debajo de 1x: Cambia de estrategia o reduce costos
```

{@ insight cost_projection @}

**Cuando aumentar el presupuesto:**

Aumenta tu presupuesto SOLO cuando:
1. Estas consistentemente en 2x+ ROI por 2+ meses
2. Mas gasto aumentaria directamente los ingresos (por ejemplo, mas creditos API = mas capacidad de clientes)
3. El aumento esta vinculado a un flujo de ingresos especifico y probado

**Cuando NO aumentar el presupuesto:**
- "Creo que esta nueva herramienta ayudara" (prueba alternativas gratuitas primero)
- "Todos dicen que tienes que gastar dinero para ganar dinero" (no en esta etapa)
- "Un VPS mas grande hara mi servicio mas rapido" (es la velocidad realmente el cuello de botella?)
- Aun no has alcanzado 1x de ROI (arregla los ingresos, no el gasto)

**La escalera de crecimiento:**

```
$200/mes  -> Probando el concepto (meses 1-3)
$500/mes  -> Escalando lo que funciona (meses 4-6)
$1000/mes -> Multiples flujos de ingresos (meses 6-12)
$2000+/mes -> Operacion de negocio completa (ano 2+)

Cada paso requiere probar ROI en el nivel actual primero.
```

> **Error Comun:** Tratar los {= regional.currency_symbol | fallback("$") =}200 como una "inversion" que no necesita retornar dinero inmediatamente. No. Este es un experimento con un plazo de 90 dias. Si {= regional.currency_symbol | fallback("$") =}200/mes no generan {= regional.currency_symbol | fallback("$") =}200/mes en ingresos dentro de 90 dias, algo de la estrategia necesita cambiar. El dinero, el mercado, la oferta — algo no esta funcionando. Se honesto contigo mismo.

### Punto de Control de la Leccion 5

Ahora deberias tener:
- [ ] Un presupuesto mensual de ~$200 asignado en cuatro niveles
- [ ] Cuentas API creadas con limites de gasto establecidos
- [ ] Decisiones de infraestructura tomadas (solo local vs. local + VPS)
- [ ] Un stack de herramientas seleccionado (mayormente tiers gratuitos para empezar)
- [ ] Objetivos de ROI: 3x dentro de 90 dias
- [ ] Una regla clara: aumentar presupuesto solo despues de probar ROI

*En el curso completo de STREETS, el Modulo E (Manual de Ejecucion) incluye una plantilla de dashboard financiero que rastrea tu gasto, ingresos y ROI por motor de ingresos en tiempo real — para que siempre sepas que flujos son rentables y cuales necesitan ajuste.*

---

## Leccion 6: Tu Documento de Stack Soberano

*"Todo negocio tiene un plan. Este es el tuyo — y cabe en dos paginas."*

### El Entregable

Esta es la cosa mas importante que crearas en el Modulo S. Tu Documento de Stack Soberano es una referencia unica que captura todo sobre tu infraestructura generadora de ingresos. Lo consultaras durante el resto del curso STREETS, lo actualizaras a medida que tu configuracion evolucione, y lo usaras para tomar decisiones con cabeza fria sobre que construir y que omitir.

Crea un nuevo archivo. Markdown, Google Doc, pagina de Notion, texto plano — lo que sea que realmente vayas a mantener. Usa la plantilla de abajo, llenando cada campo con los numeros y decisiones de las Lecciones 1-5.

### La Plantilla

{? if computed.profile_completeness != "0" ?}
> **Ventaja inicial:** 4DA ya detecto algunas de tus especificaciones de hardware e informacion del stack. Busca las pistas prellenadas abajo — te ahorraran tiempo al llenar la plantilla.
{? endif ?}

Copia esta plantilla completa y llenala. Cada campo. Sin saltar nada.

```markdown
# Documento de Stack Soberano
# [Tu Nombre o Nombre del Negocio]
# Creado: [Fecha]
# Ultima Actualizacion: [Fecha]

---

## 1. INVENTARIO DE HARDWARE

### Maquina Principal
- **Tipo:** [Desktop / Laptop / Mac / Servidor]
- **CPU:** [Modelo] — [X] nucleos, [X] hilos
- **RAM:** [X] GB [DDR4/DDR5]
- **GPU:** [Modelo] — [X] GB VRAM (o "Ninguna — solo inferencia en CPU")
- **Almacenamiento:** [X] GB SSD libres / [X] GB totales
- **SO:** [Distro Linux / version macOS / version Windows]

### Red
- **Descarga:** [X] Mbps
- **Subida:** [X] Mbps
- **Latencia a APIs cloud:** [X] ms
- **Fiabilidad del ISP:** [Estable / Cortes ocasionales / No confiable]

### Capacidad de Tiempo de Actividad
- **Puede funcionar 24/7:** [Si / No — razon]
- **UPS:** [Si / No]
- **Acceso remoto:** [SSH / RDP / Tailscale / Ninguno]

### Costo Mensual de Infraestructura
- **Electricidad (estimado 24/7):** $[X]/mes
- **Internet:** $[X]/mes (porcion de negocio)
- **Costo total fijo de infraestructura:** $[X]/mes

---

## 2. STACK DE LLM

### Modelos Locales (via Ollama)
| Modelo | Tamano | Tokens/seg | Caso de Uso |
|--------|--------|-----------|-------------|
| [ej., llama3.1:8b] | [X]B | [X] tok/s | [ej., Clasificacion, extraccion] |
| [ej., mistral:7b] | [X]B | [X] tok/s | [ej., Resumen, borradores] |
| [ej., deepseek-coder] | [X]B | [X] tok/s | [ej., Generacion de codigo] |

### Modelos API (para salida de calidad critica)
| Proveedor | Modelo | Presupuesto Mensual | Caso de Uso |
|-----------|--------|---------------------|-------------|
| [ej., Anthropic] | [Claude 3.5 Sonnet] | $[X] | [ej., Contenido orientado al cliente] |
| [ej., OpenAI] | [GPT-4o-mini] | $[X] | [ej., Respaldo de procesamiento de volumen] |

### Estrategia de Inferencia
- **Lo local maneja:** [X]% de solicitudes ([listar tareas])
- **La API maneja:** [X]% de solicitudes ([listar tareas])
- **Costo combinado estimado por 1M tokens:** $[X]

---

## 3. PRESUPUESTO MENSUAL

| Categoria | Asignacion | Real (actualizar mensualmente) |
|-----------|-----------|-------------------------------|
| Creditos API | $[X] | $[  ] |
| Infraestructura (VPS, dominio, email) | $[X] | $[  ] |
| Herramientas (analiticas, email marketing) | $[X] | $[  ] |
| Reserva | $[X] | $[  ] |
| **Total** | **$[X]** | **$[  ]** |

### Objetivo de Ingresos
- **Mes 1-3:** $[X]/mes (minimo: cubrir costos)
- **Mes 4-6:** $[X]/mes
- **Mes 7-12:** $[X]/mes

---

## 4. ESTADO LEGAL

- **Estado laboral:** [Empleado / Freelance / Entre trabajos]
- **Clausula de PI revisada:** [Si / No / N/A]
- **Nivel de riesgo de clausula de PI:** [Limpio / Turbio — necesita revision / Restrictivo]
- **Entidad de negocio:** [LLC / Ltd / Empresa Individual / Aun ninguna]
  - **Estado/Pais:** [Donde esta registrada]
  - **EIN/ID Fiscal:** [Obtenido / Pendiente / No necesario aun]
- **Procesamiento de pagos:** [Stripe / Lemon Squeezy / Otro] — [Activo / Pendiente]
- **Cuenta bancaria de negocio:** [Abierta / Pendiente / Usando personal (arregla esto)]
- **Politica de privacidad:** [Hecha / Aun no — URL: ___]
- **Terminos de servicio:** [Hechos / Aun no — URL: ___]

---

## 5. INVENTARIO DE TIEMPO

- **Horas disponibles por semana para proyectos de ingresos:** [X] horas
  - **Mananas entre semana:** [X] horas
  - **Noches entre semana:** [X] horas
  - **Fines de semana:** [X] horas
- **Zona horaria:** [Tu zona horaria]
- **Mejores bloques de trabajo profundo:** [ej., "Sabado 6am-12pm, noches entre semana 8-10pm"]

### Plan de Asignacion de Tiempo
| Actividad | Horas/semana |
|-----------|-------------|
| Construir/programar | [X] |
| Marketing/ventas | [X] |
| Trabajo/entrega para clientes | [X] |
| Aprendizaje/experimentacion | [X] |
| Administracion (facturacion, email, etc.) | [X] |

> Regla: Nunca asignes mas del 70% del tiempo disponible.
> La vida pasa. El burnout es real. Deja margen.

---

## 6. INVENTARIO DE HABILIDADES

### Habilidades Principales (cosas que podrias ensenarle a otros)
1. [Habilidad] — [anos de experiencia]
2. [Habilidad] — [anos de experiencia]
3. [Habilidad] — [anos de experiencia]

### Habilidades Secundarias (competente pero no experto)
1. [Habilidad]
2. [Habilidad]
3. [Habilidad]

### Explorando (aprendiendo ahora o quieres aprender)
1. [Habilidad]
2. [Habilidad]

### Combinaciones Unicas
Que hace TU combinacion de habilidades inusual? (Esto se convierte en tu foso en el Modulo T)
- [ej., "Conozco tanto Rust COMO estandares de datos de salud — muy pocas personas tienen ambos"]
- [ej., "Puedo construir apps full-stack Y entiendo logistica de cadena de suministro de una carrera anterior"]
- [ej., "Soy fluido en 3 idiomas Y puedo programar — puedo servir mercados no angloparlantes que la mayoria de herramientas dev ignoran"]

---

## 7. RESUMEN DEL STACK SOBERANO

### Lo Que Puedo Ofrecer Hoy
(Basado en hardware + habilidades + tiempo, que podrias vender ESTA SEMANA si alguien preguntara?)
1. [ej., "Procesamiento local de documentos — extraer datos de PDFs de forma privada"]
2. [ej., "Scripts de automatizacion personalizados para [dominio especifico]"]
3. [ej., "Redaccion tecnica / documentacion"]

### Lo Que Estoy Construyendo
(Basado en el framework completo de STREETS — llena esto a medida que avances en el curso)
1. [Motor de Ingresos 1 — del Modulo R]
2. [Motor de Ingresos 2 — del Modulo R]
3. [Motor de Ingresos 3 — del Modulo R]

### Restricciones Clave
(Se honesto — estas no son debilidades, son parametros)
- [ej., "Solo 10 horas/semana disponibles"]
- [ej., "Sin GPU — solo inferencia en CPU, dependere de APIs para tareas LLM"]
- [ej., "Contrato de trabajo es restrictivo — necesito mantenerme en dominios no relacionados"]
- [ej., "No basado en EE.UU. — algunas opciones de pago/legales son limitadas"]

---

*Este documento es una referencia viva. Actualizalo mensualmente.*
*Proxima fecha de revision: [Fecha + 30 dias]*
```

{? if dna.primary_stack ?}
> **Prellenado desde tu ADN de Desarrollador:**
> - **Stack principal:** {= dna.primary_stack | fallback("Not detected") =}
> - **Intereses:** {= dna.interests | fallback("Not detected") =}
> - **Resumen de identidad:** {= dna.identity_summary | fallback("Not yet profiled") =}
{? if dna.blind_spots ?}> - **Puntos ciegos a vigilar:** {= dna.blind_spots | fallback("None detected") =}
{? endif ?}
{? elif stack.primary ?}
> **Prellenado desde stack detectado:** Tus tecnologias principales son {= stack.primary | fallback("not yet detected") =}. {? if stack.adjacent ?}Habilidades adyacentes: {= stack.adjacent | fallback("none detected") =}.{? endif ?} Usa estas para llenar el Inventario de Habilidades arriba.
{? endif ?}

{@ insight t_shape @}

### Como Usar Este Documento

1. **Antes de iniciar cualquier proyecto nuevo:** Revisa tu Stack Soberano. Tienes el hardware, tiempo, habilidades y presupuesto para ejecutar?
2. **Antes de comprar cualquier cosa:** Revisa tu asignacion de presupuesto. Esta compra esta en el plan?
3. **Revision mensual:** Actualiza la columna "Real" en tu presupuesto. Actualiza numeros de ingresos. Ajusta asignaciones basado en lo que esta funcionando.
4. **Cuando alguien pregunte a que te dedicas:** Tu seccion "Lo Que Puedo Ofrecer Hoy" es tu pitch instantaneo.
5. **Cuando estes tentado a perseguir una nueva idea brillante:** Revisa tus restricciones. Esto cabe dentro de tu tiempo, habilidades y hardware? Si no, agregalo a "Lo Que Estoy Construyendo" para despues.

### El Ejercicio de Una Hora

Pon un temporizador de 60 minutos. Llena cada campo de la plantilla. No lo pienses demasiado. No investigues extensamente. Escribe lo que sabes ahora mismo. Puedes actualizarlo despues.

Los campos que no puedes llenar? Esos son tus tareas pendientes para esta semana:
- Numeros de benchmark vacios? Ejecuta el script de benchmark de la Leccion 2.
- No tienes entidad de negocio? Inicia el proceso de registro de la Leccion 4.
- No tienes procesamiento de pagos? Configura Stripe de la Leccion 4.
- Inventario de habilidades en blanco? Dedica 15 minutos a listar todo por lo que te han pagado en los ultimos 5 anos.

> **Error Comun:** Dedicar 3 horas a hacer el documento "perfecto" en vez de 1 hora a hacerlo "terminado." El Documento de Stack Soberano es una referencia de trabajo, no un plan de negocios para inversionistas. Nadie lo vera excepto tu. La precision importa. El formato no.

### Punto de Control de la Leccion 6

Ahora deberias tener:
- [ ] Un Documento de Stack Soberano completo guardado en algun lugar que realmente vayas a abrir
- [ ] Las seis secciones llenadas con numeros reales (no aspiracionales)
- [ ] Una lista clara de tareas pendientes para brechas en tu configuracion
- [ ] Una fecha establecida para tu primera revision mensual (30 dias a partir de ahora)

---

## Modulo S: Completo

{? if progress.completed("MODULE_S") ?}
> **Modulo S completado.** Has terminado {= progress.completed_count | fallback("1") =} de {= progress.total_count | fallback("7") =} modulos STREETS. {? if progress.completed_modules ?}Completados: {= progress.completed_modules | fallback("S") =}.{? endif ?}
{? endif ?}

### Lo Que Has Construido en Dos Semanas

Mira lo que ahora tienes que no tenias cuando empezaste:

1. **Un inventario de hardware** mapeado a capacidades generadoras de ingresos — no solo especificaciones en una etiqueta.
2. **Un stack de LLM local de grado produccion** con Ollama, con benchmark en tu hardware real, configurado para cargas de trabajo reales.
3. **Una ventaja de privacidad** que entiendes como comercializar — con lenguaje especifico para audiencias especificas.
4. **Una base legal y financiera** — entidad de negocio (o plan), procesamiento de pagos, cuenta bancaria, estrategia fiscal.
5. **Un presupuesto controlado** con objetivos claros de ROI y un plazo de 90 dias para probar el modelo.
6. **Un Documento de Stack Soberano** que captura todo lo anterior en una referencia unica que usaras para cada decision de aqui en adelante.

Esto es mas de lo que la mayoria de los desarrolladores jamas configura. En serio. La mayoria de las personas que quieren generar ingresos paralelos saltan directo a "construir algo genial" y luego se preguntan por que no pueden cobrar. Tu ahora tienes la infraestructura para cobrar.

Pero infraestructura sin direccion es solo un hobby caro. Necesitas saber hacia donde apuntar este stack.

{@ temporal market_timing @}

### Lo Que Sigue: Modulo T — Fosos Tecnicos

El Modulo S te dio la base. El Modulo T responde la pregunta critica: **como construyes algo que los competidores no puedan copiar facilmente?**

Esto es lo que cubre el Modulo T:

- **Pipelines de datos propietarios** — como crear datasets a los que solo tu tienes acceso, legal y eticamente
- **Configuraciones de modelos personalizadas** — fine-tuning e ingenieria de prompts que producen calidad de salida que otros no pueden igualar con configuraciones predeterminadas
- **Stacks de habilidades compuestas** — por que "Python + salud" supera a "Python + JavaScript" para ingresos, y como identificar tu combinacion unica
- **Barreras tecnicas de entrada** — disenos de infraestructura que le tomarian meses replicar a un competidor
- **La Auditoria de Foso** — un framework para evaluar si tu proyecto tiene una ventaja defendible o es solo otro servicio commoditizado

La diferencia entre un desarrollador que gana $500/mes y uno que gana $5,000/mes rara vez es habilidad. Son los fosos. Cosas que hacen tu oferta dificil de replicar, incluso si alguien tiene el mismo hardware y los mismos modelos.

### La Hoja de Ruta Completa de STREETS

| Modulo | Titulo | Enfoque | Duracion |
|--------|--------|---------|----------|
| **S** | Configuracion Soberana | Infraestructura, legal, presupuesto | Semanas 1-2 (completo) |
| **T** | Fosos Tecnicos | Ventajas defendibles, activos propietarios | Semanas 3-4 |
| **R** | Motores de Ingresos | Guias especificas de monetizacion con codigo | Semanas 5-8 |
| **E** | Manual de Ejecucion | Secuencias de lanzamiento, precios, primeros clientes | Semanas 9-10 |
| **E** | Ventaja en Evolucion | Mantenerse adelante, deteccion de tendencias, adaptacion | Semanas 11-12 |
| **T** | Automatizacion Tactica | Automatizar operaciones para ingresos pasivos | Semanas 13-14 |
| **S** | Apilando Flujos | Multiples fuentes de ingresos, estrategia de portafolio | Semanas 15-16 |

El Modulo R (Motores de Ingresos) es donde se gana la mayor parte del dinero. Pero sin S y T, estas construyendo sobre arena.

---

**Listo para la guia completa?**

Has visto la base. La has construido tu mismo. Ahora obtiene el sistema completo.

**Obtiene STREETS Core** — el curso completo de 16 semanas con los siete modulos, plantillas de codigo de motores de ingresos, dashboards financieros y la comunidad privada de desarrolladores construyendo ingresos en sus propios terminos.

*Tu equipo. Tus reglas. Tus ingresos.*
