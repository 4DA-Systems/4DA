# Modulo E: Manual de Ejecucion

**Curso STREETS de Ingresos para Desarrolladores — Modulo de Pago**
*Semanas 9-10 | 6 Lecciones | Entregable: Tu Primer Producto, En Vivo y Aceptando Pagos*

> "De la idea al despliegue en 48 horas. Sin pensarlo demasiado."

---

Tienes la infraestructura (Modulo S). Tienes el foso defensivo (Modulo T). Tienes los disenos del motor de ingresos (Modulo R). Ahora es momento de lanzar.

Este modulo es al que la mayoria de los desarrolladores nunca llegan — no porque sea dificil, sino porque siguen puliendo su base de codigo, refactorizando su arquitectura, ajustando su paleta de colores. Estan haciendo todo excepto lo que importa: poner un producto frente a un ser humano que pueda pagar por el.

Lanzar es una habilidad. Como cualquier habilidad, se vuelve mas facil con la practica y peor con la demora. Cuanto mas esperas, mas dificil se vuelve. Cuanto mas lanzas, menos miedo da. Tu primer lanzamiento sera desordenado. Ese es el punto.

Al final de estas dos semanas, tendras:

- Una idea de producto validada, probada contra senales reales de demanda
- Un producto en vivo, desplegado y accesible a traves de un dominio real
- Procesamiento de pagos aceptando dinero real
- Al menos un lanzamiento publico en una plataforma donde se reune tu audiencia objetivo
- Un sistema de metricas post-lanzamiento para guiar tus proximos movimientos

Sin hipoteticos. Sin "en teoria." Un producto real, en vivo en internet, capaz de generar ingresos.

{? if progress.completed("R") ?}
Completaste el Modulo R — ya tienes disenos de motores de ingresos listos para ejecutar. Este modulo convierte uno de esos disenos en un producto en vivo.
{? else ?}
Si aun no has completado el Modulo R, puedes usar este modulo de todas formas — pero tener un diseno de motor de ingresos listo hara que el sprint de 48 horas sea significativamente mas fluido.
{? endif ?}

{@ mirror execution_readiness @}

Vamos a construirlo.

---

## Leccion 1: El Sprint de 48 Horas

*"Sabado por la manana a domingo por la noche. Un producto. Cero excusas."*

### Por Que 48 Horas

La Ley de Parkinson dice que el trabajo se expande para llenar el tiempo disponible. Date 6 meses para construir un producto y pasaras 5 meses deliberando y 1 mes en un frenesi estresado. Date 48 horas y tomaras decisiones, cortaras alcance sin piedad y lanzaras algo real.

La restriccion de 48 horas no se trata de construir algo perfecto. Se trata de construir algo que exista. La existencia vence a la perfeccion cada vez, porque un producto en vivo genera datos — quien visita, quien hace clic, quien paga, quien se queja — y los datos te dicen que construir despues.

Cada producto exitoso para desarrolladores que he estudiado siguio este patron: lanza rapido, aprende rapido, itera rapido. Los que fracasaron? Todos tienen archivos README hermosos y cero usuarios.

Aqui esta tu manual minuto a minuto.

### Dia 1 — Sabado

#### Bloque de la Manana (4 horas): Validar la Demanda

Antes de escribir una sola linea de codigo, necesitas evidencia de que alguien ademas de ti quiere esto. No certeza — evidencia. La diferencia importa. La certeza es imposible. La evidencia es alcanzable en 4 horas.

**Paso 1: Verificacion del Volumen de Busqueda (45 minutos)**

Ve a estas fuentes y busca tu idea de producto y terminos relacionados:

- **Google Trends** (https://trends.google.com) — Gratis. Muestra el interes de busqueda relativo a lo largo del tiempo. Quieres ver una linea plana o ascendente, no una descendente.
- **Ahrefs Free Webmaster Tools** (https://ahrefs.com/webmaster-tools) — Gratis con verificacion de sitio. Muestra volumenes de palabras clave.
- **Ubersuggest** (https://neilpatel.com/ubersuggest/) — El nivel gratuito da 3 busquedas/dia. Muestra volumen de busqueda, dificultad y terminos relacionados.
- **AlsoAsked** (https://alsoasked.com) — Nivel gratuito. Muestra datos de "La gente tambien pregunta" de Google. Revela que preguntas esta haciendo la gente realmente.

Lo que estas buscando:

```
BUENAS senales:
- 500+ busquedas mensuales para tu palabra clave principal
- Tendencia ascendente en los ultimos 12 meses
- Multiples preguntas de "La gente tambien pregunta" sin buenas respuestas
- Palabras clave long-tail relacionadas con baja competencia

MALAS senales:
- Interes de busqueda en declive
- Cero volumen de busqueda (nadie esta buscando esto)
- Dominado por empresas masivas en la pagina 1
- Sin variacion en los terminos de busqueda (demasiado estrecho)
```

Ejemplo real: Supongamos que tu idea de motor de ingresos del Modulo R es una "biblioteca de componentes Tailwind CSS para dashboards SaaS."

```
Busqueda: "tailwind dashboard components" — 2,900/mes, tendencia ascendente
Busqueda: "tailwind admin template" — 6,600/mes, estable
Busqueda: "react dashboard template tailwind" — 1,300/mes, ascendente
Relacionado: "shadcn dashboard", "tailwind analytics components"

Veredicto: Demanda fuerte. Multiples angulos de palabras clave. Proceder.
```

Otro ejemplo: Supongamos que tu idea es un "anonimizador de archivos de log basado en Rust."

```
Busqueda: "log file anonymizer" — 90/mes, plano
Busqueda: "anonymize log files" — 140/mes, plano
Busqueda: "PII removal from logs" — 320/mes, ascendente
Relacionado: "GDPR log compliance", "scrub PII from logs"

Veredicto: Nicho pero creciendo. El angulo de "PII removal" tiene mas volumen
que el angulo de "anonymizer". Reformula tu posicionamiento.
```

**Paso 2: Mineria de Hilos de la Comunidad (60 minutos)**

Ve a donde los desarrolladores piden cosas y busca tu espacio de problema:

- **Reddit:** Busca en r/webdev, r/reactjs, r/selfhosted, r/SideProject, r/programming, y subreddits de nicho relevantes a tu dominio
- **Hacker News:** Usa https://hn.algolia.com para buscar discusiones pasadas
- **GitHub Issues:** Busca issues en repositorios populares relacionados con tu espacio
- **Stack Overflow:** Busca preguntas con muchos votos pero respuestas aceptadas insatisfactorias
- **Servidores de Discord:** Revisa servidores de comunidades de desarrolladores relevantes

Lo que estas documentando:

```markdown
## Resultados de la Mineria de Hilos

### Hilo 1
- **Fuente:** Reddit r/reactjs
- **URL:** [enlace]
- **Titulo:** "Is there a good Tailwind dashboard kit that isn't $200?"
- **Votos positivos:** 147
- **Comentarios:** 83
- **Citas clave:**
  - "Everything on the market is either free and ugly, or $200+ and overkill"
  - "I just need 10-15 well-designed components, not 500"
  - "Would pay $49 for something that actually looks good out of the box"
- **Conclusion:** Sensibilidad al precio en $200+, disposicion a pagar en $29-49

### Hilo 2
- ...
```

Encuentra al menos 5 hilos. Si no puedes encontrar 5 hilos donde la gente pide algo en el espacio de tu producto, eso es una senal de advertencia seria. O la demanda no existe, o estas buscando con los terminos equivocados. Prueba diferentes palabras clave antes de abandonar la idea.

**Paso 3: Auditoria de Competidores (45 minutos)**

Busca lo que ya existe. Esto no es desalentador — es validador. Los competidores significan que hay un mercado. No tener competidores generalmente significa que no hay mercado, no que encontraste un oceano azul.

Para cada competidor, documenta:

```markdown
## Auditoria de Competidores

### Competidor 1: [Nombre]
- **URL:** [enlace]
- **Precio:** $XX
- **Lo que hacen bien:** [cosas especificas]
- **Lo que es malo:** [quejas especificas de resenas/hilos]
- **Sus resenas:** [revisa G2, resenas de ProductHunt, menciones en Reddit]
- **Tu angulo:** [como lo harias diferente]

### Competidor 2: [Nombre]
- ...
```

El oro esta en "lo que es malo." Cada queja sobre un competidor es una solicitud de funcion para tu producto. La gente literalmente te dice que construir y cuanto cobrar.

**Paso 4: La Prueba de "10 Personas Pagarian" (30 minutos)**

Esta es la puerta de validacion final. Necesitas encontrar evidencia de que al menos 10 personas pagarian dinero por esto. No "expresaron interes." No "dijeron que era genial." Pagarian.

Fuentes de evidencia:
- Hilos de Reddit donde la gente dice "Yo pagaria por X" (senal mas fuerte)
- Productos competidores con clientes que pagan (prueba que el mercado paga)
- Productos en Gumroad/Lemon Squeezy en tu espacio con conteos de ventas visibles
- Repositorios de GitHub con 1,000+ estrellas que resuelven un problema relacionado (la gente valora esto lo suficiente para dar estrella)
- Tu propia audiencia si tienes una (tuitea, envia DM a 10 personas, pregunta directamente)

Si pasas esta prueba: procede. Construyelo.

Si fallas esta prueba: pivotea tu angulo, no toda tu idea. La demanda podria existir en un espacio adyacente. Prueba diferente posicionamiento antes de abandonar.

> **Hablando en Serio:** La mayoria de los desarrolladores se saltan la validacion por completo porque quieren programar. Pasaran 200 horas construyendo algo que nadie pidio, y luego se preguntan por que nadie compra. Estas 4 horas de investigacion te ahorraran 196 horas de esfuerzo desperdiciado. No te saltes esto. El codigo es la parte facil.

#### Bloque de la Tarde (4 horas): Construir el MVP

Has validado la demanda. Tienes investigacion de competidores. Sabes lo que la gente quiere y lo que les falta a las soluciones existentes. Ahora construye la version minima que resuelva el problema central.

{? if profile.gpu.exists ?}
Con una GPU en tu equipo ({= profile.gpu.model | fallback("tu GPU") =}), considera ideas de producto que aprovechen la inferencia de IA local — herramientas de procesamiento de imagenes, utilidades de analisis de codigo, pipelines de generacion de contenido. Las funcionalidades impulsadas por GPU son un diferenciador genuino que la mayoria de los desarrolladores independientes no pueden ofrecer.
{? endif ?}

**La Regla de las 3 Funcionalidades**

Tu v0.1 tiene exactamente 3 funcionalidades. No 4. No 7. Tres.

Como elegirlas:
1. Cual es la UNICA cosa que hace tu producto? (Funcionalidad 1 — el nucleo)
2. Que lo hace usable? (Funcionalidad 2 — generalmente autenticacion, o guardar/exportar, o configuracion)
3. Que lo hace digno de pagar sobre las alternativas? (Funcionalidad 3 — tu diferenciador)

Todo lo demas va en una lista de "v0.2" que no tocas este fin de semana.

Ejemplo real — una biblioteca de componentes de dashboard Tailwind:
1. **Nucleo:** 12 componentes de dashboard listos para produccion (graficos, tablas, tarjetas de estadisticas, navegacion)
2. **Usable:** Fragmentos de codigo para copiar y pegar con vista previa en vivo
3. **Diferenciador:** Modo oscuro integrado, componentes disenados para funcionar juntos (no una coleccion aleatoria)

Ejemplo real — una herramienta CLI de limpieza de PII en logs:
1. **Nucleo:** Detectar y redactar PII de archivos de log (correos, IPs, nombres, SSNs)
2. **Usable:** Funciona como pipe de CLI (`cat logs.txt | pii-scrub > clean.txt`)
3. **Diferenciador:** Archivo de reglas configurable, maneja 15+ formatos de log automaticamente

{@ insight stack_fit @}

**Prepara el Proyecto**

Usa LLMs para acelerar, no reemplazar, tu trabajo. Aqui esta el flujo de trabajo practico:

{? if stack.contains("react") ?}
Ya que tu stack principal incluye React, el scaffold de app web a continuacion es tu camino mas rapido. Ya conoces las herramientas — enfoca tus 48 horas en la logica del producto, no en aprender un nuevo framework.
{? elif stack.contains("rust") ?}
Ya que tu stack principal incluye Rust, el scaffold de herramienta CLI a continuacion es tu camino mas rapido. Las herramientas CLI de Rust tienen excelente distribucion (binario unico, multiplataforma) y las audiencias de desarrolladores respetan la historia de rendimiento.
{? elif stack.contains("python") ?}
Ya que tu stack principal incluye Python, considera una herramienta CLI o un servicio API. Python se lanza rapido con FastAPI o Typer, y el ecosistema PyPI te da distribucion instantanea a millones de desarrolladores.
{? endif ?}

```bash
# Scaffold de una app web (herramienta SaaS, biblioteca de componentes con sitio de docs, etc.)
pnpm create vite@latest my-product -- --template react-ts
cd my-product
pnpm install

# Agregar Tailwind CSS (lo mas comun para productos de desarrolladores)
pnpm install -D tailwindcss @tailwindcss/vite

# Agregar enrutamiento si necesitas multiples paginas
pnpm install react-router-dom

# Estructura del proyecto — mantenla plana para una construccion de 48 horas
mkdir -p src/components src/pages src/lib
```

```bash
# Scaffold de una herramienta CLI (para utilidades de desarrolladores)
cargo init my-tool
cd my-tool

# Dependencias comunes para herramientas CLI
cargo add clap --features derive    # Parseo de argumentos
cargo add serde --features derive   # Serializacion
cargo add serde_json                # Manejo de JSON
cargo add anyhow                    # Manejo de errores
cargo add regex                     # Coincidencia de patrones
```

```bash
# Scaffold de un paquete npm (para bibliotecas/utilidades)
mkdir my-package && cd my-package
pnpm init
pnpm install -D typescript tsup vitest
mkdir src
```

**El Flujo de Trabajo con LLM para Construir**

{? if settings.has_llm ?}
Tienes un LLM configurado ({= settings.llm_provider | fallback("local") =} / {= settings.llm_model | fallback("tu modelo") =}). Usalo como tu programador en pareja durante el sprint — acelera significativamente el scaffolding y la generacion de boilerplate.
{? endif ?}

No le pidas al LLM que construya todo tu producto. Eso produce codigo generico y fragil. En su lugar:

1. **Tu** escribes la arquitectura: estructura de archivos, flujo de datos, interfaces clave
2. **LLM** genera boilerplate: componentes repetitivos, funciones de utilidad, definiciones de tipos
3. **Tu** escribes la logica central: la parte que hace diferente a tu producto
4. **LLM** genera tests: tests unitarios, casos limite, tests de integracion
5. **Tu** revisas y editas todo: tu nombre esta en este producto

Trabajo en paralelo mientras programas: abre un segundo chat con LLM y haz que redacte el copy de tu landing page, README y documentacion. Los editaras en la noche, pero los primeros borradores estaran listos.

**Disciplina de Tiempo**

```
2:00 PM — Funcionalidad 1 (funcionalidad central): 2 horas
           Si no funciona para las 4 PM, recorta alcance.
4:00 PM — Funcionalidad 2 (usabilidad): 1 hora
           Mantenlo simple. Lanza el pulido despues.
5:00 PM — Funcionalidad 3 (diferenciador): 1 hora
           Esto es lo que te hace digno de pagar. Enfocate aqui.
6:00 PM — DEJA DE PROGRAMAR. No necesita ser perfecto.
```

> **Error Comun:** "Solo una funcionalidad mas antes de parar." Asi es como los proyectos de fin de semana se convierten en proyectos de un mes. Las 3 funcionalidades son tu alcance. Si piensas en una gran idea durante la construccion, escríbela en tu lista de v0.2 y sigue avanzando. Puedes agregarla la proxima semana despues de tener clientes que pagan.

#### Bloque de la Noche (2 horas): Escribir la Landing Page

Tu landing page tiene un trabajo: convencer a un visitante de pagar. No necesita ser hermosa. Necesita ser clara.

**La Landing Page de 5 Secciones**

Cada landing page exitosa de producto para desarrolladores sigue esta estructura. No la reinventes:

```
Seccion 1: TITULAR + SUBTITULAR
  - Lo que hace en 8 palabras o menos
  - Para quien es y que resultado obtienen

Seccion 2: EL PROBLEMA
  - 3 puntos de dolor que tu cliente objetivo reconoce
  - Usa su lenguaje exacto de tu mineria de hilos

Seccion 3: LA SOLUCION
  - Capturas de pantalla o ejemplos de codigo de tu producto
  - 3 funcionalidades mapeadas a los 3 puntos de dolor anteriores

Seccion 4: PRECIOS
  - Uno o dos niveles. Mantenlo simple para la v0.1.
  - Opcion de facturacion anual si es una suscripcion.

Seccion 5: CTA (Llamada a la Accion)
  - Un boton. "Empezar", "Comprar Ahora", "Descargar".
  - Repite el beneficio central.
```

**Ejemplo Real de Copy — Kit de Dashboard Tailwind:**

```markdown
# Seccion 1
## DashKit — Componentes de Dashboard Tailwind Listos para Produccion
Lanza tu dashboard SaaS en horas, no semanas.
12 componentes para copiar y pegar. Modo oscuro. $29.

# Seccion 2
## El Problema
- Los kits de UI genericos te dan 500 componentes pero cero cohesion
- Construir UIs de dashboard desde cero toma 40+ horas
- Las opciones gratuitas parecen Bootstrap de 2018

# Seccion 3
## Lo Que Obtienes
- **12 componentes** disenados para funcionar juntos (no una coleccion aleatoria)
- **Modo oscuro** integrado — activa con un prop
- **Codigo para copiar y pegar** — sin npm install, sin dependencias, sin ataduras
[captura de pantalla de ejemplos de componentes]

# Seccion 4
## Precios
**DashKit** — $29 unico pago
- Todos los 12 componentes con codigo fuente
- Actualizaciones gratuitas por 12 meses
- Usa en proyectos ilimitados

**DashKit Pro** — $59 unico pago
- Todo en DashKit
- 8 plantillas de pagina completa (analiticas, CRM, admin, configuracion)
- Archivos de diseno Figma
- Solicitudes de funcionalidades prioritarias

# Seccion 5
## Lanza tu dashboard este fin de semana.
[Comprar DashKit — $29]
```

**Ejemplo Real de Copy — Limpiador de PII en Logs:**

```markdown
# Seccion 1
## ScrubLog — Elimina PII de Archivos de Log en Segundos
Cumplimiento GDPR para tus logs. Un comando.

# Seccion 2
## El Problema
- Tus logs contienen correos, IPs y nombres que no deberias estar almacenando
- La redaccion manual toma horas y se le escapan cosas
- Las herramientas empresariales cuestan $500/mes y requieren un doctorado para configurar

# Seccion 3
## Como Funciona
```bash
cat server.log | scrublog > clean.log
```
- Detecta 15+ patrones de PII automaticamente
- Reglas personalizadas via configuracion YAML
- Maneja formatos JSON, Apache, Nginx y texto plano
[captura de terminal mostrando antes/despues]

# Seccion 4
## Precios
**Personal** — Gratis
- 5 patrones de PII, 1 formato de log

**Pro** — $19/mes
- Todos los 15+ patrones de PII
- Todos los formatos de log
- Reglas personalizadas
- Compartir configuracion en equipo

# Seccion 5
## Deja de almacenar PII que no necesitas.
[Obtener ScrubLog Pro — $19/mes]
```

**Flujo de Trabajo con LLM para Copy:**

1. Alimenta al LLM con tu auditoria de competidores y resultados de mineria de hilos
2. Pidele que redacte copy de landing page usando la plantilla de 5 secciones
3. Edita sin piedad: reemplaza cada frase vaga con una especifica
4. Leelo en voz alta. Si alguna frase te hace sentir incomodo, reescribela.

**Construyendo la Landing Page:**

Para un sprint de 48 horas, no construyas una landing page personalizada desde cero. Usa una de estas:

{? if stack.contains("react") ?}
- **Tu app React** — Ya que trabajas en React, haz que la landing page sea la pagina de inicio sin sesion de tu app o agrega una ruta de marketing en Next.js. Cero costo de cambio de contexto.
{? endif ?}
- **El propio sitio de tu producto** — Si es una app web, haz que la landing page sea la pagina de inicio sin sesion
- **Astro + Tailwind** — Sitio estatico, se despliega en Vercel en 2 minutos, extremadamente rapido
- **Next.js** — Si tu producto ya es React, agrega una ruta de pagina de marketing
- **Framer** (https://framer.com) — Constructor visual, exporta codigo limpio, nivel gratuito disponible
- **Carrd** (https://carrd.co) — $19/ano, sitios de una pagina super simples

```bash
# El camino mas rapido: sitio estatico Astro
pnpm create astro@latest my-product-site
cd my-product-site
pnpm install
# Agregar Tailwind
pnpm astro add tailwind
```

Deberias tener una landing page con copy para el final del sabado. No necesita ilustraciones personalizadas. No necesita animaciones. Necesita palabras claras y un boton de compra.

### Dia 2 — Domingo

#### Bloque de la Manana (3 horas): Desplegar

Tu producto necesita estar en vivo en internet en una URL real. No localhost. No una URL de preview de Vercel con un hash aleatorio. Un dominio real, con HTTPS, que puedas compartir y la gente pueda visitar.

**Paso 1: Desplegar la Aplicacion (60 minutos)**

{? if computed.os_family == "windows" ?}
Ya que estas en Windows, asegurate de que WSL2 este disponible si tus herramientas de despliegue lo requieren. La mayoria de las herramientas CLI de despliegue (Vercel, Fly.io) funcionan nativamente en Windows, pero algunos scripts asumen rutas Unix.
{? elif computed.os_family == "macos" ?}
En macOS, todas las CLIs de despliegue se instalan limpiamente via Homebrew o descarga directa. Estas en el camino de despliegue mas fluido.
{? elif computed.os_family == "linux" ?}
En Linux, tienes el entorno de despliegue mas flexible. Todas las herramientas CLI funcionan nativamente, y tambien puedes autoalojar en tu propia maquina si tienes una IP estatica y quieres ahorrar en costos de hosting.
{? endif ?}

Elige tu plataforma de despliegue basandote en lo que construiste:

**Sitio estatico / SPA (biblioteca de componentes, landing page, sitio de docs):**
```bash
# Vercel — el camino mas rapido para sitios estaticos y Next.js
pnpm install -g vercel
vercel

# Te hara preguntas. Di si a todo.
# Tu sitio esta en vivo en ~60 segundos.
```

**App web con backend (herramienta SaaS, servicio API):**
```bash
# Railway — simple, buen nivel gratuito, maneja bases de datos
# https://railway.app
# Conecta tu repositorio de GitHub y despliega.

# O Fly.io — mas control, despliegue en edge global
# https://fly.io
curl -L https://fly.io/install.sh | sh
fly launch
fly deploy
```

**Herramienta CLI / paquete npm:**
```bash
# registro npm
npm publish

# O distribuye como binario via GitHub Releases
# Usa cargo-dist para proyectos Rust
cargo install cargo-dist
cargo dist init
cargo dist build
# Sube binarios a la release de GitHub
```

**Paso 2: Comprar un Dominio (30 minutos)**

Un dominio real cuesta $12/ano. Si no puedes invertir $12 en tu negocio, no vas en serio con tener un negocio.

**Donde comprar:**
- **Namecheap** (https://namecheap.com) — $8-12/ano para .com, buena gestion de DNS
- **Cloudflare Registrar** (https://dash.cloudflare.com) — Precios a costo (frecuentemente $9-10/ano para .com), excelente DNS
- **Porkbun** (https://porkbun.com) — A menudo el mas barato para el primer ano, buena interfaz

**Consejos para nombrar dominios:**
- Mas corto es mejor. 2 silabas ideal, 3 maximo.
- `.com` sigue ganando en confianza. `.dev` y `.io` estan bien para herramientas de desarrolladores.
- Verifica la disponibilidad en tu registrador, no en GoDaddy (ellos hacen front-running de busquedas).
- No pases mas de 15 minutos eligiendo. El nombre importa menos de lo que crees.

```bash
# Apunta tu dominio a Vercel
# En el dashboard de Vercel: Settings > Domains > Add your domain
# Luego en la configuracion DNS de tu registrador, agrega:
# A record: @ -> 76.76.21.21
# CNAME record: www -> cname.vercel-dns.com

# O si usas Cloudflare para DNS:
# Solo agrega los mismos registros en el panel DNS de Cloudflare
# SSL es automatico con tanto Vercel como Cloudflare
```

**Paso 3: Monitoreo Basico (30 minutos)**

Necesitas saber dos cosas: si el sitio esta activo, y si la gente lo visita.

**Monitoreo de uptime (gratis):**
- **Better Uptime** (https://betteruptime.com) — El nivel gratuito monitorea 10 URLs cada 3 minutos
- **UptimeRobot** (https://uptimerobot.com) — El nivel gratuito monitorea 50 URLs cada 5 minutos

```
Configura monitoreo para:
1. La URL de tu landing page
2. El endpoint de salud de tu app (si aplica)
3. La URL de tu webhook de pagos (critico — necesitas saber si los pagos se rompen)
```

**Analiticas (respetuosas con la privacidad):**

No uses Google Analytics. Tu audiencia de desarrolladores lo bloquea, es excesivo para un producto nuevo, y es un riesgo de privacidad.

- **Plausible** (https://plausible.io) — $9/mes, privacidad primero, un script de una linea
- **Fathom** (https://usefathom.com) — $14/mes, privacidad primero, ligero
- **Umami** (https://umami.is) — Gratis y autoalojado, o $9/mes en la nube

```html
<!-- Plausible — una linea en tu <head> -->
<script defer data-domain="yourdomain.com"
  src="https://plausible.io/js/script.js"></script>

<!-- Umami — una linea en tu <head> -->
<script defer
  src="https://your-umami-instance.com/script.js"
  data-website-id="your-website-id"></script>
```

> **Hablando en Serio:** Si, $9/mes para analiticas en un producto que no ha ganado dinero aun se siente innecesario. Pero no puedes mejorar lo que no puedes medir. El primer mes de datos de analiticas te dira mas sobre tu mercado que un mes de adivinanzas. Si $9/mes rompe tu presupuesto, autoaloja Umami gratis en Railway.

#### Bloque de la Tarde (2 horas): Configurar Pagos

Si tu producto no puede aceptar dinero, es un proyecto de hobby. Configurar pagos toma menos tiempo del que la mayoria de los desarrolladores creen — alrededor de 20-30 minutos para el flujo basico.

{? if regional.country ?}
> **Procesadores de pago recomendados para {= regional.country | fallback("tu pais") =}:** {= regional.payment_processors | fallback("Stripe, Lemon Squeezy, PayPal") =}. Las opciones a continuacion son globalmente disponibles, pero verifica que tu procesador preferido soporte pagos en {= regional.currency | fallback("tu moneda local") =}.
{? endif ?}

**Opcion A: Lemon Squeezy (Recomendado para Productos Digitales)**

Lemon Squeezy (https://lemonsqueezy.com) maneja procesamiento de pagos, impuestos sobre ventas, IVA y entrega digital en una sola plataforma. Es el camino mas rapido de cero a aceptar pagos.

Por que Lemon Squeezy sobre Stripe para tu primer producto:
- Actua como Merchant of Record — ellos manejan impuestos sobre ventas, IVA y cumplimiento por ti
- Paginas de checkout integradas — no se necesita trabajo de frontend
- Entrega digital integrada — sube tus archivos, ellos manejan el acceso
- 5% + $0.50 por transaccion (mas alto que Stripe, pero te ahorra horas de dolores de cabeza con impuestos)

Tutorial de configuracion:
1. Registrate en https://app.lemonsqueezy.com
2. Crea una Tienda (el nombre de tu negocio)
3. Agrega un Producto:
   - Nombre, descripcion, precio
   - Sube archivos para entrega digital (si aplica)
   - Configura claves de licencia (si vendes software)
4. Obtiene tu URL de checkout — esto es a lo que apunta tu boton de "Comprar"
5. Configura un webhook para automatizacion post-compra

```javascript
// Manejador de webhook de Lemon Squeezy (Node.js/Express)
// POST /api/webhooks/lemonsqueezy

import crypto from 'crypto';

const WEBHOOK_SECRET = process.env.LEMONSQUEEZY_WEBHOOK_SECRET;

export async function handleLemonSqueezyWebhook(req, res) {
  // Verificar firma del webhook
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

      // Enviar email de bienvenida, otorgar acceso, crear clave de licencia, etc.
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

**Opcion B: Stripe (Mas Control, Mas Trabajo)**

Stripe (https://stripe.com) te da mas control pero requiere que manejes el cumplimiento fiscal por separado. Mejor para SaaS con facturacion compleja.

```javascript
// Sesion de Stripe Checkout (Node.js)
// Crea una pagina de checkout alojada

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
          unit_amount: 5900, // $59.00 en centavos
        },
        quantity: 1,
      },
    ],
    mode: 'payment', // 'subscription' para recurrente
    success_url: `${process.env.DOMAIN}/success?session_id={CHECKOUT_SESSION_ID}`,
    cancel_url: `${process.env.DOMAIN}/pricing`,
    customer_email: req.body.email, // Pre-llenar si lo tienes
  });

  return res.json({ url: session.url });
}

// Manejador de webhook de Stripe
export async function handleStripeWebhook(req, res) {
  const sig = req.headers['stripe-signature'];

  let event;
  try {
    event = stripe.webhooks.constructEvent(
      req.body, // cuerpo crudo, no JSON parseado
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

**Para Ambas Plataformas — Prueba Antes de Lanzar:**

```bash
# Lemon Squeezy: Usa el modo de prueba en el dashboard
# Activa "Test mode" en la esquina superior derecha del dashboard de Lemon Squeezy
# Usa numero de tarjeta: 4242 4242 4242 4242, cualquier fecha futura, cualquier CVC

# Stripe: Usa claves API de modo prueba
# Tarjeta de prueba: 4242 4242 4242 4242
# Tarjeta de prueba que se rechaza: 4000 0000 0000 0002
# Tarjeta de prueba que requiere autenticacion: 4000 0025 0000 3155
```

Recorre todo el flujo de compra tu mismo en modo prueba. Haz clic en el boton de compra, completa el checkout, verifica que el webhook se dispare, verifica que el acceso se otorgue. Si algun paso falla en modo prueba, fallara para clientes reales.

> **Error Comun:** "Configurare los pagos despues, cuando tenga algunos usuarios." Esto es al reves. Configurar pagos no se trata de cobrar dinero hoy — se trata de validar si alguien pagara. Un producto sin precio es una herramienta gratuita. Un producto con precio es una prueba de negocio. El precio en si es parte de la validacion.

#### Bloque de la Noche (3 horas): Lanzar

Tu producto esta en vivo. Los pagos funcionan. La landing page es clara. Ahora necesitas que los humanos lo vean.

**La Estrategia de Lanzamiento Suave**

No hagas un "gran lanzamiento" para tu primer producto. Los grandes lanzamientos crean presion para ser perfecto, y tu v0.1 no es perfecta. En su lugar, haz un lanzamiento suave: compartelo en algunos lugares, recopila retroalimentacion, arregla problemas criticos, luego haz el gran lanzamiento en 1-2 semanas.

**Plataforma de Lanzamiento 1: Reddit (30 minutos)**

Publica en r/SideProject y un subreddit de nicho relevante a tu producto.

Plantilla de publicacion en Reddit:

```markdown
Title: I built [lo que hace] in a weekend — [beneficio clave]

Body:
Hey [subreddit],

I've been frustrated with [el problema] for a while, so I built
[nombre del producto] this weekend.

**What it does:**
- [Funcionalidad 1 — el valor central]
- [Funcionalidad 2]
- [Funcionalidad 3]

**What makes it different from [competidor]:**
[Un parrafo honesto sobre tu diferenciador]

**Pricing:**
[Se transparente. "$29 one-time" o "Free tier + $19/mo Pro"]

I'd love feedback. What am I missing? What would make this
useful for your workflow?

[Enlace al producto]
```

Reglas para publicaciones en Reddit:
- Se genuinamente util, no vendedor
- Responde a cada comentario individual (esto no es opcional)
- Acepta las criticas con gracia — la retroalimentacion negativa es la mas valiosa
- No hagas astroturfing (votos falsos, multiples cuentas). Te atraparan y te banearan.

**Plataforma de Lanzamiento 2: Hacker News (30 minutos)**

Si tu producto es tecnico e interesante, publica un Show HN. En la seccion de "Detalles tecnicos", menciona tu stack ({= stack.primary | fallback("tu stack principal") =}) y explica por que lo elegiste — los lectores de HN aman las decisiones tecnicas informadas.

Plantilla de Show HN:

```markdown
Title: Show HN: [Nombre del Producto] – [lo que hace en <70 caracteres]

Body:
[Nombre del producto] is [una oracion explicando lo que hace].

I built this because [motivacion genuina — que problema estabas resolviendo
para ti mismo].

Technical details:
- Built with [stack]
- [Decision tecnica interesante y por que]
- [Que hace notable la implementacion]

Try it: [URL]

Feedback welcome. I'm particularly interested in [pregunta especifica para
la audiencia de HN].
```

Consejos para HN:
- Publica entre las 7-9 AM hora del este de EE.UU. (mayor trafico)
- El titulo importa mas que cualquier otra cosa. Se especifico y tecnico.
- Los lectores de HN respetan la sustancia tecnica sobre el marketing pulido
- Responde a los comentarios inmediatamente en las primeras 2 horas. La velocidad de comentarios afecta el ranking.
- No supliques por votos. Solo publica e interactua.

**Plataforma de Lanzamiento 3: Twitter/X (30 minutos)**

Escribe un hilo de lanzamiento de construccion en publico:

```
Tweet 1 (Gancho):
I built [producto] in 48 hours this weekend.

It [resuelve problema especifico] for [audiencia especifica].

Here's what I shipped, what I learned, and the real numbers. Thread:

Tweet 2 (El Problema):
The problem:
[Describe el punto de dolor en 2-3 oraciones]
[Incluye una captura de pantalla o ejemplo de codigo mostrando el dolor]

Tweet 3 (La Solucion):
So I built [nombre del producto].

[Captura de pantalla/GIF del producto en accion]

It does three things:
1. [Funcionalidad 1]
2. [Funcionalidad 2]
3. [Funcionalidad 3]

Tweet 4 (Detalle Tecnico):
Tech stack for the nerds:
- [Frontend]
- [Backend]
- [Hosting — menciona la plataforma especifica]
- [Pagos — menciona Lemon Squeezy/Stripe]
- Total cost to run: $XX/month

Tweet 5 (Precios):
Pricing:
[Precios claros, igual que en la landing page]
[Enlace al producto]

Tweet 6 (Peticion):
Would love feedback from anyone who [describe al usuario objetivo].

What am I missing? What would make this a must-have for you?
```

**Plataforma de Lanzamiento 4: Comunidades Relevantes (30 minutos)**

Identifica 2-3 comunidades donde tu audiencia objetivo pasa el tiempo:

- Servidores de Discord (comunidades de desarrolladores, servidores especificos de frameworks)
- Comunidades de Slack (muchas comunidades de desarrollo de nicho tienen grupos de Slack)
- Dev.to / Hashnode (escribe una publicacion corta de "Construi esto")
- Indie Hackers (https://indiehackers.com) — disenado especificamente para esto
- Grupos relevantes de Telegram o WhatsApp

**Primeras 48 Horas Despues del Lanzamiento — Que Observar:**

```
Metricas a rastrear:
1. Visitantes unicos (de analiticas)
2. Tasa de clic de landing page a checkout (deberia ser 2-5%)
3. Tasa de conversion de checkout a compra (deberia ser 1-3%)
4. Tasa de rebote (por encima del 80% significa que tu titular/hero esta mal)
5. Fuentes de trafico (de donde vienen tus visitantes?)
6. Comentarios y retroalimentacion (cualitativo — que esta diciendo la gente?)

Matematica de ejemplo:
- 500 visitantes en 48 horas (razonable desde Reddit + HN + Twitter)
- 3% hace clic en "Comprar" = 15 visitas al checkout
- 10% completa la compra = 1-2 ventas
- A $29/venta = $29-58 en tu primer fin de semana

Eso no es dinero para la jubilacion. Es dinero de VALIDACION.
$29 de un desconocido en internet prueba que tu producto tiene valor.
```

No entres en panico si obtienes cero ventas en las primeras 48 horas. Mira tu embudo:
- Cero visitantes? Tu distribucion es el problema, no tu producto.
- Visitantes pero cero clics en "Comprar"? Tu copy o precio es el problema.
- Clics en "Comprar" pero cero compras completadas? Tu flujo de checkout esta roto o tu precio es demasiado alto para el valor percibido.

Cada uno de estos tiene una solucion diferente. Por eso las metricas importan.

### Tu Turno

1. **Bloquea el tiempo.** Abre tu calendario ahora mismo y bloquea el proximo sabado de 8 AM a 8 PM y domingo de 8 AM a 8 PM. Etiquetalo "Sprint de 48 Horas." Tratalo como un vuelo que no puedes reprogramar.

2. **Elige tu idea.** Escoge un motor de ingresos del Modulo R. Escribe el alcance de 3 funcionalidades para tu v0.1. Si no puedes elegir uno, elige el que puedas explicar a un no-desarrollador en una oracion.
{? if dna.primary_stack ?}
   Tu camino de ejecucion mas fuerte es construir algo con {= dna.primary_stack | fallback("tu stack principal") =} — lanza mas rapido donde ya tienes experiencia profunda.
{? endif ?}

3. **Pre-trabajo.** Antes del sabado, crea cuentas en:
   - Vercel, Railway, o Fly.io (despliegue)
   - Lemon Squeezy o Stripe (pagos)
   - Namecheap, Cloudflare, o Porkbun (dominio)
   - Plausible, Fathom, o Umami (analiticas)
   - Better Uptime o UptimeRobot (monitoreo)

   Haz esto en una noche de la semana para que el sabado sea pura construccion, no creacion de cuentas.

4. **Prepara tus plataformas de lanzamiento.** Si no tienes una cuenta de Reddit con algo de karma, empieza a participar en subreddits relevantes esta semana. Las cuentas que solo publican autopromocion se marcan. Si no tienes una cuenta de Hacker News, crea una y participa en algunas discusiones primero.

---

## Leccion 2: La Mentalidad de "Lanza, Luego Mejora"

*"v0.1 con 3 funcionalidades le gana a v1.0 que nunca se lanza."*

### La Trampa del Perfeccionismo

Los desarrolladores son unicamente susceptibles a un modo de fallo especifico: construir en privado para siempre. Sabemos como se ve el "buen codigo." Sabemos que nuestro v0.1 no es buen codigo. Entonces refactorizamos. Agregamos manejo de errores. Escribimos mas tests. Mejoramos la arquitectura. Hacemos todo excepto lo unico que importa: mostrarlo a humanos.

Aqui hay una verdad que te ahorrara miles de horas: **tus clientes no leen tu codigo fuente.** No les importa tu arquitectura. No les importa tu cobertura de tests. Les importa una cosa: esto resuelve mi problema?

Un producto con codigo espagueti que resuelve un problema real generara dinero. Un producto con arquitectura hermosa que no resuelve ningun problema no generara nada.

Esto no es una excusa para escribir mal codigo. Es una declaracion de prioridades. Lanza primero. Refactoriza despues. La refactorizacion sera mejor informada por datos de uso real de todos modos.

### Como se Desarrolla "Lanza, Luego Mejora"

Considera este escenario: un desarrollador lanza un paquete de plantillas de Notion para gerentes de ingenieria de software. Asi se ve en el lanzamiento:

- 5 plantillas (no 50)
- Una pagina de Gumroad con un parrafo de descripcion y 3 capturas de pantalla
- Sin sitio web personalizado
- Sin lista de correo
- Sin seguidores en redes sociales
- Precio: $29

Lo publican en Reddit y Twitter. Esa es toda la estrategia de marketing.

Resultados del Mes 1:
- ~170 ventas a $29 = ~$5,000
- Despues del corte de Gumroad (10%): ~$4,500
- Tiempo invertido: ~30 horas en total (construir plantillas + escribir descripciones)
- Tasa por hora efectiva: ~$150/hora

Fue "perfecto"? No. Las plantillas tenian inconsistencias de formato. Algunas descripciones eran genericas. A los clientes no les importo. Les importo que les ahorraba de construir las plantillas ellos mismos.

Para el mes 3, basandose en la retroalimentacion de clientes, el desarrollador:
- Arreglo los problemas de formato
- Agrego mas plantillas (las que los clientes pidieron especificamente)
- Subio el precio a $39 (los clientes existentes obtuvieron actualizaciones gratis)
- Creo un nivel "Pro" con un video tutorial acompanante

El producto que lanzaron era peor en todos los aspectos que el producto que tenian 90 dias despues. Pero la version de 90 dias solo existio porque la version de lanzamiento genero la retroalimentacion y los ingresos para guiar el desarrollo.

> **NOTA:** Para validacion del mundo real del modelo "lanza feo, mejora rapido": Josh Comeau pre-vendio $550K de su curso CSS para Desarrolladores JavaScript en la primera semana (Fuente: failory.com). Wes Bos ha generado $10M+ en ventas totales de cursos para desarrolladores usando lanzamientos iterativos (Fuente: foundershut.com). Ambos empezaron con productos v1 imperfectos e iteraron basandose en retroalimentacion real de clientes.

### Los Primeros 10 Clientes Te Dicen Todo

Tus primeros 10 clientes que pagan son las personas mas importantes en tu negocio. No por su dinero — 10 ventas a $29 son $290, lo que te compra comestibles. Son importantes porque son voluntarios para tu equipo de desarrollo de producto.

Que hacer con tus primeros 10 clientes:

1. **Envia un email de agradecimiento personal.** No automatizado. Personal. "Hola, vi que compraste [producto]. Gracias. Estoy desarrollando esto activamente — hay algo que desees que hiciera que no hace?"

2. **Lee cada respuesta.** Algunos no responderan. Algunos responderan con "se ve genial, gracias." Pero 2-3 de cada 10 escribiran parrafos sobre lo que quieren. Esos parrafos son tu hoja de ruta.

3. **Busca patrones.** Si 3 de cada 10 personas piden la misma funcionalidad, construyela. Eso es una senal de demanda del 30% de clientes que pagan. Ninguna encuesta te dara datos tan buenos.

4. **Pregunta sobre su disposicion a pagar mas.** "Estoy planeando un nivel Pro con [funcionalidad X]. Te pareceria justo $49?" Directo. Especifico. Te da datos de precios.

```
Plantilla de email para los primeros 10 clientes:

Asunto: Pregunta rapida sobre [nombre del producto]

Hola [nombre],

Vi que adquiriste [nombre del producto] — gracias por ser
uno de los primeros clientes.

Estoy construyendo esto activamente y lanzando actualizaciones
semanalmente. Pregunta rapida: cual es la UNICA cosa que deseas
que hiciera que no hace?

No hay respuestas equivocadas. Incluso si parece una peticion
grande, quiero escucharla.

Gracias,
[Tu nombre]
```

### Como Manejar la Retroalimentacion Negativa

Tu primera pieza de retroalimentacion negativa se sentira personal. No es personal. Son datos.

**Marco para procesar retroalimentacion negativa:**

```
1. PAUSA. No respondas por 30 minutos. Tu reaccion emocional
   no es util.

2. CATEGORIZA la retroalimentacion:
   a) Reporte de bug — arreglalo. Agradeceles.
   b) Solicitud de funcionalidad — agregala al backlog. Agradeceles.
   c) Queja de precio — anotala. Verifica si es un patron.
   d) Queja de calidad — investiga. Es valida?
   e) Troll/irrazonable — ignora. Sigue adelante.

3. RESPONDE (solo para a, b, c, d):
   "Gracias por la retroalimentacion. [Reconoce el problema especifico].
   Estoy [arreglandolo ahora / agregandolo a la hoja de ruta / investigandolo].
   Te avisare cuando se resuelva."

4. ACTUA. Si prometiste arreglar algo, arreglalo en una semana.
   Nada construye lealtad mas rapido que mostrar a los clientes que su
   retroalimentacion lleva a cambios reales.
```

> **Hablando en Serio:** Alguien te dira que tu producto es basura. Dolera. Pero si tu producto esta en vivo y generando dinero, ya has hecho algo que la mayoria de los desarrolladores nunca hacen. La persona criticando desde la seccion de comentarios no ha lanzado nada. Tu si. Sigue lanzando.

### El Ciclo de Iteracion Semanal

Despues del lanzamiento, tu flujo de trabajo se convierte en un ciclo cerrado:

```
Lunes:     Revisa las metricas de la semana pasada y retroalimentacion de clientes
Martes:    Planifica la mejora de esta semana (UNA cosa, no cinco)
Miercoles: Construye la mejora
Jueves:    Prueba y despliega la mejora
Viernes:   Escribe una publicacion de changelog/actualizacion
Fin de semana: Marketing — una publicacion de blog, una publicacion social, una interaccion comunitaria

Repite.
```

La palabra clave es UNA mejora por semana. No una renovacion de funcionalidades. No un rediseno. Una cosa que hace el producto ligeramente mejor para tus clientes existentes. En 12 semanas, eso son 12 mejoras guiadas por datos de uso real. Tu producto despues de 12 semanas de este ciclo sera dramaticamente mejor que cualquier cosa que hubieras podido disenar en aislamiento.

### Los Ingresos Validan Mas Rapido que las Encuestas

Las encuestas mienten. No intencionalmente — la gente simplemente es mala prediciendo su propio comportamiento. "Pagarias $29 por esto?" obtiene respuestas faciles de "si." Pero "aqui esta la pagina de checkout, ingresa tu tarjeta de credito" obtiene respuestas honestas.

Por eso lanzas con pagos desde el dia uno:

| Metodo de Validacion | Tiempo para Senal | Calidad de la Senal |
|---|---|---|
| Encuesta / sondeo | 1-2 semanas | Baja (la gente miente) |
| Landing page con registro de email | 1-2 semanas | Media (interes, no compromiso) |
| Landing page con precio pero sin checkout | 1 semana | Media-Alta (aceptacion de precio) |
| **Producto en vivo con checkout real** | **48 horas** | **La mas alta (comportamiento real de compra)** |

El precio de $0 no revela nada. El precio de $29 lo revela todo.

### Tu Turno

1. **Escribe tu compromiso de "lanzamiento feo."** Abre un archivo de texto y escribe: "Lanzare [nombre del producto] el [fecha] aunque no sea perfecto. Alcance v0.1: [3 funcionalidades]. No agregare la Funcionalidad 4 antes del lanzamiento." Firmalo (metaforicamente). Consultalo cuando surja la urgencia de pulir.

2. **Redacta tu email para los primeros 10 clientes.** Escribe la plantilla de email de agradecimiento personal ahora, antes de tener clientes. Cuando llegue la primera venta, quieres enviarlo dentro de la hora.

3. **Configura tu rastreador de iteraciones.** Crea una hoja de calculo simple o pagina de Notion con columnas: Semana | Mejora Realizada | Impacto en Metricas | Retroalimentacion de Clientes. Esto se convierte en tu registro de decisiones para que construir despues.

---

## Leccion 3: Psicologia de Precios para Productos de Desarrolladores

*"$0 no es un precio. Es una trampa."*

### Por Que Lo Gratuito Es Caro

La verdad mas contraintuitiva en vender productos para desarrolladores: **los usuarios gratuitos te cuestan mas que los clientes que pagan.**

Usuarios gratuitos:
- Envian mas solicitudes de soporte (no tienen nada en juego)
- Demandan mas funcionalidades (se sienten con derecho porque no estan pagando)
- Proporcionan retroalimentacion menos util ("esta genial" no es accionable)
- Se van a tasas mas altas (no hay costo de cambio)
- Cuentan a menos personas sobre tu producto (las cosas gratis tienen bajo valor percibido)

Clientes que pagan:
- Estan invertidos en tu exito (quieren que su compra sea una buena decision)
- Proporcionan retroalimentacion especifica y accionable (quieren que el producto mejore)
- Son mas faciles de retener (ya decidieron pagar; la inercia trabaja a tu favor)
- Refieren a otros mas frecuentemente (recomendar algo que pagaste valida tu compra)
- Respetan tu tiempo (entienden que estas operando un negocio)

La unica razon para ofrecer un nivel gratuito es como mecanismo de generacion de leads para el nivel de pago. Si tu nivel gratuito es tan bueno que la gente nunca se actualiza, no tienes un nivel gratuito — tienes un producto gratuito con un boton de donacion.

> **Error Comun:** "Lo hare gratuito para obtener usuarios primero, luego cobrare." Esto casi nunca funciona. Los usuarios que atraes a $0 esperan $0 para siempre. Cuando agregas un precio, se van. Los usuarios que hubieran pagado $29 desde el dia uno nunca encontraron tu producto porque lo posicionaste como una herramienta gratuita. Atrajiste a la audiencia equivocada.

{@ insight cost_projection @}

### Los Niveles de Precios para Productos de Desarrolladores

Despues de analizar cientos de productos exitosos para desarrolladores, estos puntos de precio funcionan consistentemente. Todos los precios a continuacion estan en USD — si estas fijando precios en {= regional.currency | fallback("tu moneda local") =}, ajusta para el poder adquisitivo local y las normas del mercado.

**Nivel 1: $9-29 — Herramientas y Utilidades para Desarrolladores**

Los productos en este rango resuelven un problema especifico y estrecho. Una sola compra, usalo hoy.

```
Ejemplos:
- Extension de VS Code con funcionalidades premium: $9-15
- Herramienta CLI con funcionalidades pro: $15-19
- Herramienta SaaS de proposito unico: $9-19/mes
- Biblioteca de componentes pequena: $19-29
- Extension de DevTools del navegador: $9-15

Psicologia del comprador: Territorio de compra impulsiva. El desarrollador
la ve, reconoce el problema, la compra sin preguntar a su gerente.
No se necesita aprobacion de presupuesto. Tarjeta de credito -> listo.

Insight clave: A este precio, tu landing page debe convertir en
menos de 2 minutos. El comprador no leera una lista larga de funcionalidades.
Muestra el problema, muestra la solucion, muestra el precio.
```

**Nivel 2: $49-99 — Plantillas, Kits y Herramientas Completas**

Los productos en este rango ahorran tiempo significativo. Multiples componentes trabajando juntos.

```
Ejemplos:
- Kit completo de plantillas UI: $49-79
- Boilerplate SaaS con auth, facturacion, dashboards: $79-99
- Conjunto completo de iconos/ilustraciones: $49-69
- Toolkit CLI multiproposito: $49
- Biblioteca wrapper de API con docs extensos: $49-79

Psicologia del comprador: Compra considerada. El desarrollador evalua
por 5-10 minutos. Compara con alternativas. Calcula el tiempo ahorrado.
"Si esto me ahorra 10 horas y valoro mi tiempo en $50/hora,
$79 es obvia decision."

Insight clave: Necesitas un punto de comparacion. Muestra el tiempo/esfuerzo
que toma construir esto desde cero vs. comprar tu kit.
Incluye testimonios si los tienes.
```

**Nivel 3: $149-499 — Cursos, Soluciones Completas, Plantillas Premium**

Los productos en este rango transforman una habilidad o proporcionan un sistema completo.

```
Ejemplos:
- Curso en video (10+ horas): $149-299
- Kit de inicio SaaS con codigo fuente completo + video tutorial: $199-299
- Biblioteca de componentes empresarial: $299-499
- Toolkit completo para desarrolladores (multiples herramientas): $199
- Codebase completo + lecciones de "Construye X Desde Cero": $149-249

Psicologia del comprador: Compra de inversion. El comprador necesita justificar
el gasto (para si mismo o su gerente). Necesitan prueba social,
previews detallados, y una narrativa clara de ROI.

Insight clave: En este nivel, ofrece garantia de devolucion de dinero.
Reduce la ansiedad de compra y aumenta las conversiones. Las tasas de devolucion
para productos digitales de desarrolladores son tipicamente 3-5%.
El aumento de conversiones supera con creces las devoluciones.
```

### La Estrategia de Precios de 3 Niveles

Si tu producto lo soporta, ofrece tres niveles de precio. Esto no es aleatorio — explota un sesgo cognitivo bien documentado llamado el "efecto de escenario central." Cuando se presentan tres opciones, la mayoria de la gente elige la del medio.

```
Estructura de niveles:

BASICO          PRO (destacado)       EQUIPO/EMPRESA
$29             $59                   $149
Funcionalidades  Todo en Basico       Todo en Pro
centrales        + funcionalidades    + funcionalidades de equipo
                 premium              + licencia comercial
                 + soporte prioritario

Distribucion de conversion (tipica):
- Basico: 20-30%
- Pro: 50-60% <- este es tu objetivo
- Equipo: 10-20%
```

**Como disenar los niveles:**

1. Empieza con el nivel **Pro**. Este es el producto que realmente quieres vender, al precio que refleja su valor. Disena este primero.

2. Crea el nivel **Basico** eliminando funcionalidades de Pro. Elimina lo suficiente para que Basico resuelva el problema pero Pro lo resuelva *bien*. Basico deberia sentirse ligeramente frustrante — usable, pero claramente limitado.

3. Crea el nivel **Equipo** agregando funcionalidades a Pro. Licencias multi-puesto, derechos de uso comercial, soporte prioritario, marca personalizada, acceso al codigo fuente, archivos Figma, etc.

**Ejemplo real de pagina de precios:**

```
DashKit

STARTER — $29                    PRO — $59                        EQUIPO — $149
                                 * Mas Popular                    Ideal para agencias

* 12 componentes centrales       * Todo en Starter                * Todo en Pro
* React + TypeScript              * 8 plantillas de pagina completa * Hasta 5 miembros del equipo
* Modo oscuro                     * Archivos de diseno Figma       * Licencia comercial
* npm install                     * Tabla de datos avanzada          (proyectos de cliente ilimitados)
* 6 meses de actualizaciones     * Integracion de biblioteca       * Soporte prioritario
                                   de graficos                     * Actualizaciones de por vida
                                 * 12 meses de actualizaciones     * Opciones de marca personalizada
                                 * Solicitudes de funcionalidades
                                   prioritarias

[Obtener Starter]                [Obtener Pro]                    [Obtener Equipo]
```

### Anclaje de Precios

El anclaje es el sesgo cognitivo donde el primer numero que la gente ve influye en su percepcion de los numeros subsecuentes. Usalo eticamente:

1. **Muestra la opcion cara primero** (a la derecha en layouts occidentales). Ver $149 hace que $59 se sienta razonable.

2. **Muestra calculos de "horas ahorradas".**
   ```
   "Construir estos componentes desde cero toma ~40 horas.
   A $50/hora, eso son $2,000 de tu tiempo.
   DashKit Pro: $59."
   ```

3. **Usa reformulacion "por dia" para suscripciones.**
   ```
   "$19/mes" -> "Menos de $0.63/dia"
   "$99/ano" -> "$8.25/mes" o "$0.27/dia"
   ```

4. **Descuento por facturacion anual.** Ofrece 2 meses gratis en planes anuales. Esto es estandar y esperado. La facturacion anual reduce la desercion en un 30-40% porque la cancelacion requiere una decision consciente en un unico punto de renovacion, no una decision mensual continua.

```
Mensual: $19/mes
Anual: $190/ano (ahorra $38 — 2 meses gratis)

Muestra como:
Mensual: $19/mes
Anual: $15.83/mes (facturado anualmente a $190)
```

### Pruebas A/B de Precios

Probar precios es valioso pero delicado. Asi es como hacerlo sin ser deshonesto:

**Enfoques aceptables:**
- Prueba diferentes precios en diferentes canales de lanzamiento (Reddit obtiene $29, Product Hunt obtiene $39, ve cual convierte mejor)
- Cambia tu precio despues de 2 semanas y compara tasas de conversion
- Ofrece un descuento de lanzamiento ("$29 esta semana, $39 despues") y ve si la urgencia cambia el comportamiento
- Prueba diferentes estructuras de niveles (2 niveles vs 3 niveles) en diferentes periodos de tiempo

**No aceptable:**
- Mostrar diferentes precios a diferentes visitantes en la misma pagina al mismo tiempo (discriminacion de precios, erosiona la confianza)
- Cobrar mas basandose en ubicacion o deteccion de navegador (la gente habla, y te atraparan)

### Cuando Subir Precios

Sube tus precios cuando cualquiera de estas sea verdad:

1. **La tasa de conversion esta por encima del 5%.** Estas demasiado barato. Una tasa de conversion saludable para una landing page de producto para desarrolladores es 1-3%. Por encima del 5% significa que casi todos los que ven el precio estan de acuerdo en que es una buena oferta — lo que significa que estas dejando dinero en la mesa.

2. **Nadie se ha quejado del precio.** Si cero personas de 100 dicen que es demasiado caro, esta demasiado barato. Un producto saludable tiene alrededor del 20% de visitantes pensando que el precio es alto. Eso significa que el 80% piensa que es justo o una buena oferta.

3. **Has agregado funcionalidades significativas desde el lanzamiento.** Lanzaste a $29 con 3 funcionalidades. Ahora tienes 8 funcionalidades y mejor documentacion. El producto vale mas. Cobra mas.

4. **Tienes testimonios y prueba social.** El valor percibido aumenta con la prueba social. Una vez que tienes 5+ resenas positivas, tu producto vale mas en la mente del comprador.

**Como subir precios:**
- Anuncia el aumento de precio 1-2 semanas antes ("El precio sube de $29 a $39 el [fecha]")
- Mantiene a los clientes existentes al precio anterior
- Esto no es turbio — es practica estandar y tambien crea urgencia para los indecisos

> **Hablando en Serio:** La mayoria de los desarrolladores fijan precios un 50-200% por debajo. Tu producto de {= regional.currency_symbol | fallback("$") =}29 probablemente vale {= regional.currency_symbol | fallback("$") =}49. Tu producto de {= regional.currency_symbol | fallback("$") =}49 probablemente vale {= regional.currency_symbol | fallback("$") =}79. Se esto porque los desarrolladores se anclan a su propia disposicion a pagar (baja — somos tacanos con las herramientas) en lugar de la disposicion a pagar del cliente (mas alta — estan comprando una solucion a un problema que les cuesta tiempo). Sube tus precios antes de lo que crees.

### Tu Turno

1. **Pon precio a tu producto.** Basandote en el analisis de niveles anterior, elige un punto de precio para tu lanzamiento v0.1. Anotalo. Si te sientes incomodo porque parece "demasiado alto," probablemente estas en el rango correcto. Si se siente comodo, agrega 50%.

2. **Disena tu pagina de precios.** Usando la plantilla de 3 niveles, disena el copy de tu pagina de precios. Identifica que funcionalidades van en cada nivel. Identifica tu nivel "destacado" (el que quieres que la mayoria de la gente compre).

3. **Calcula tus matematicas.** Completa:
   - Precio por venta: {= regional.currency_symbol | fallback("$") =}___
   - Ingreso mensual objetivo: {= regional.currency_symbol | fallback("$") =}___
   - Numero de ventas necesarias por mes: ___
   - Visitantes estimados de landing page necesarios (al 2% de conversion): ___
   - Ese conteo de visitantes es alcanzable con tu plan de distribucion? (Si/No)

---

## Leccion 4: Configuracion Legal Minima Viable

*"30 minutos de configuracion legal ahora te ahorran 30 horas de panico despues."*

### La Verdad Honesta Sobre la Configuracion Legal

La mayoria de los desarrolladores ignoran lo legal por completo (arriesgado) o se paralizan con ello (desperdicio). El enfoque correcto es una configuracion legal minima viable: suficiente proteccion para operar legitimamente, sin gastar $5,000 en un abogado antes de haber ganado $5.

Esto es lo que realmente necesitas antes de tu primera venta, lo que necesitas antes de tu venta numero 100, y lo que no necesitas hasta mucho despues.

### Antes de Tu Primera Venta (Hazlo Este Fin de Semana)

**1. Revisa Tu Contrato de Empleo (30 minutos)**

Si tienes un trabajo de tiempo completo, lee la clausula de PI de tu contrato de empleo antes de construir cualquier cosa. Busca especificamente:

- **Clausulas de asignacion de invenciones:** Algunos contratos dicen que todo lo que creas mientras estas empleado — incluso en tu propio tiempo — pertenece a tu empleador.
- **Clausulas de no competencia:** Algunas te restringen de trabajar en la misma industria, incluso como proyecto paralelo.
- **Politicas de trabajo secundario:** Algunas requieren aprobacion escrita para actividades comerciales externas.

```
Lo que estas buscando:

SEGURO: "Las invenciones hechas en tiempo de la empresa o usando recursos
de la empresa pertenecen a la empresa." -> Tu proyecto de fin de semana en tu
maquina personal es tuyo.

AMBIGUO: "Todas las invenciones relacionadas con el negocio actual o
anticipado de la empresa." -> Si tu proyecto paralelo esta en el mismo
dominio que tu empleador, busca asesoria legal.

RESTRICTIVO: "Todas las invenciones concebidas durante el periodo de
empleo pertenecen a la empresa." -> Esto es agresivo pero
comun en algunas empresas. Busca asesoria legal antes de proceder.
```

Estados como California, Delaware, Illinois, Minnesota, Washington y otros tienen leyes que limitan cuan ampliamente los empleadores pueden reclamar tus invenciones personales. Pero el lenguaje especifico de tu contrato importa.

> **Error Comun:** "Lo mantendre en secreto." Si tu producto se vuelve lo suficientemente exitoso como para importar, alguien lo notara. Si viola tu contrato de empleo, podrias perder el producto Y tu trabajo. 30 minutos de leer tu contrato ahora previene esto.

**2. Politica de Privacidad (15 minutos)**

Si tu producto recopila cualquier dato — incluso solo una direccion de correo para la compra — necesitas una politica de privacidad. Este es un requisito legal en la UE (GDPR), California (CCPA), y cada vez en mas lugares.

No escribas una desde cero. Usa un generador:

- **Termly** (https://termly.io/products/privacy-policy-generator/) — Nivel gratuito, responde preguntas, obtiene una politica
- **Avodocs** (https://www.avodocs.com) — Gratis, plantillas legales de codigo abierto
- **Iubenda** (https://www.iubenda.com) — Nivel gratuito, genera automaticamente basado en tu stack tecnologico

Tu politica de privacidad debe cubrir:

```markdown
# Politica de Privacidad para [Nombre del Producto]
Ultima actualizacion: [Fecha]

## Lo Que Recopilamos
- Direccion de correo electronico (para confirmacion de compra y actualizaciones del producto)
- Informacion de pago (procesada por [Lemon Squeezy/Stripe],
  nunca vemos o almacenamos los datos de tu tarjeta)
- Analiticas basicas de uso (vistas de pagina, uso de funcionalidades — via
  [Plausible/Fathom/Umami], respetuosas con la privacidad, sin cookies)

## Lo Que NO Recopilamos
- No te rastreamos por la web
- No vendemos tus datos a nadie
- No usamos cookies de publicidad

## Como Usamos Tus Datos
- Para entregar el producto que compraste
- Para enviar actualizaciones del producto y avisos importantes
- Para mejorar el producto basandonos en patrones de uso agregados

## Almacenamiento de Datos
- Tus datos se almacenan en servidores de [proveedor de hosting] en [region]
- Los datos de pago son manejados completamente por [Lemon Squeezy/Stripe]

## Tus Derechos
- Puedes solicitar una copia de tus datos en cualquier momento
- Puedes solicitar la eliminacion de tus datos en cualquier momento
- Contacto: [tu email]

## Cambios
- Te notificaremos de cambios significativos por email
```

Pon esto en `tudominio.com/privacy`. Enlazalo desde el pie de pagina de tu pagina de checkout.

**3. Terminos de Servicio (15 minutos)**

Tus terminos de servicio te protegen de reclamaciones irrazonables. Para un producto digital, son directos.

```markdown
# Terminos de Servicio para [Nombre del Producto]
Ultima actualizacion: [Fecha]

## Licencia
Cuando compras [Nombre del Producto], recibes una licencia para usarlo
con propositos [personales/comerciales].

- **Licencia individual:** Uso en tus propios proyectos (ilimitados)
- **Licencia de equipo:** Uso por hasta [N] miembros del equipo
- NO puedes redistribuir, revender o compartir credenciales de acceso

## Devoluciones
- Productos digitales: garantia de devolucion de dinero de [30 dias / 14 dias]
- Si no estas satisfecho, envia un email a [tu email] para un reembolso completo
- Sin preguntas dentro del periodo de devolucion

## Responsabilidad
- [Nombre del Producto] se proporciona "tal cual" sin garantia
- No somos responsables de danos derivados del uso del producto
- La responsabilidad maxima se limita a la cantidad que pagaste

## Soporte
- El soporte se proporciona por email en [tu email]
- Buscamos responder dentro de [48 horas / 2 dias habiles]

## Modificaciones
- Podemos actualizar estos terminos con aviso
- El uso continuado constituye aceptacion de los terminos actualizados
```

Pon esto en `tudominio.com/terms`. Enlazalo desde el pie de pagina de tu pagina de checkout.

### Antes de Tu Venta Numero 100 (Primeros Meses)

**4. Entidad Comercial (1-3 horas + tiempo de procesamiento)**

Operar como empresario individual (el predeterminado cuando vendes cosas sin formar una empresa) funciona para tus primeras ventas. Pero a medida que los ingresos crecen, quieres proteccion de responsabilidad y ventajas fiscales.

{? if regional.country ?}
> **Para {= regional.country | fallback("tu region") =}:** El tipo de entidad recomendado es una **{= regional.business_entity_type | fallback("LLC o equivalente") =}**, con costos tipicos de registro de {= regional.currency_symbol | fallback("$") =}{= regional.business_registration_cost | fallback("50-500") =}. Encuentra la seccion de tu pais a continuacion para orientacion especifica.
{? endif ?}

**Estados Unidos — LLC:**

Una LLC (Sociedad de Responsabilidad Limitada) es la opcion estandar para negocios de desarrolladores solitarios.

```
Costo: $50-500 dependiendo del estado (tarifa de presentacion)
Tiempo: 1-4 semanas para procesamiento
Donde presentar: Tu estado de residencia, a menos que haya una razon especifica
para usar Delaware o Wyoming

Presentacion por tu cuenta (mas barato):
1. Ve al sitio web del Secretario de Estado de tu estado
2. Presenta "Articles of Organization" (el formulario generalmente es de 1-2 paginas)
3. Paga la tarifa de presentacion ($50-250 dependiendo del estado)
4. Obtiene tu EIN (identificacion fiscal) de IRS.gov — gratis, instantaneo en linea

Comparacion de estados para desarrolladores solitarios:
- Wyoming: $100 presentacion, $60/ano reporte anual. Sin impuesto estatal sobre la renta.
             Bueno para privacidad (no se requiere informacion publica de miembros).
- Delaware: $90 presentacion, $300/ano impuesto anual. Popular pero no
            necesariamente mejor para desarrolladores solitarios.
- New Mexico: $50 presentacion, sin reporte anual. Mas barato de mantener.
- California: $70 presentacion, $800/ano impuesto minimo de franquicia.
              Caro. Pagas esto incluso si ganas $0.
```

**Stripe Atlas (si quieres que lo hagan por ti):**

Stripe Atlas (https://atlas.stripe.com) cuesta $500 y configura una LLC de Delaware, cuenta bancaria de EE.UU. (via Mercury), cuenta de Stripe, y proporciona guias de impuestos y legales. Si no eres de EE.UU. o simplemente quieres que alguien mas maneje el papeleo, vale los $500.

**Reino Unido — Ltd Company:**

```
Costo: GBP 12 en Companies House (https://www.gov.uk/set-up-limited-company)
Tiempo: Generalmente 24-48 horas
Continuo: Declaracion de confirmacion anual (GBP 13), presentacion de cuentas anuales

Para desarrolladores solitarios: Una Ltd company te da proteccion de responsabilidad
y eficiencia fiscal una vez que las ganancias superan ~GBP 50,000/ano.
Por debajo de eso, sole trader es mas simple.
```

**Union Europea:**

Cada pais tiene su propia estructura. Opciones comunes:
- **Alemania:** GmbH (costosa de crear) o registro de freelancer (barato)
- **Paises Bajos:** BV o eenmanszaak (empresa unipersonal)
- **Francia:** auto-entrepreneur (micro-empresa) — muy comun para desarrolladores solitarios, impuesto fijo simple
- **Estonia:** e-Residency + OUe estonio (popular con nomadas digitales, empresa EU completa por ~EUR 190)

**Australia:**

```
Sole trader: Gratis para registrarse via solicitud de ABN (https://www.abr.gov.au)
Company (Pty Ltd): AUD 538 registro con ASIC
Para desarrolladores solitarios: Empieza como sole trader. Registra una empresa
cuando los ingresos justifiquen la carga contable (~AUD 100K+/ano).
```

**5. Obligaciones Fiscales**

Si estas usando Lemon Squeezy como tu plataforma de pagos, ellos manejan impuestos sobre ventas e IVA como Merchant of Record. Esta es una simplificacion masiva.

Si estas usando Stripe directamente, eres responsable de:
- **Impuesto sobre ventas de EE.UU.:** Varia por estado. Usa Stripe Tax ($0.50/transaccion) o TaxJar para automatizar.
- **IVA de la UE:** 20-27% dependiendo del pais. Requerido para ventas digitales a clientes de la UE independientemente de donde estes. Lemon Squeezy maneja esto; Stripe Tax puede automatizarlo.
- **IVA del Reino Unido:** 20%. Requerido si tus ventas en el Reino Unido superan GBP 85,000/ano.
- **Impuestos de Servicios Digitales:** Varios paises los estan imponiendo. Otra razon para usar Lemon Squeezy hasta que tu volumen justifique gestionarlo tu mismo.

{? if regional.country ?}
> **Nota fiscal para {= regional.country | fallback("tu region") =}:** {= regional.tax_note | fallback("Consulta a un profesional fiscal local para los detalles de tus obligaciones.") =}
{? endif ?}

> **Hablando en Serio:** La mayor ventaja de Lemon Squeezy sobre Stripe para un desarrollador solitario no es la pagina de checkout ni las funcionalidades. Es que manejan el cumplimiento fiscal globalmente. Los impuestos sobre ventas internacionales son una pesadilla. Lemon Squeezy cobra 5% + $0.50 por transaccion y hace que la pesadilla desaparezca. Hasta que no estes ganando {= regional.currency_symbol | fallback("$") =}5,000+/mes, el 5% vale la pena. Despues de eso, evalua si gestionar impuestos tu mismo con Stripe + TaxJar te ahorra dinero y cordura.

**6. Fundamentos de Propiedad Intelectual**

Lo que necesitas saber:

- **Tu codigo tiene copyright automaticamente** en el momento que lo escribes. No se necesita registro. Pero el registro (EE.UU.: $65 en copyright.gov) te da una posicion legal mas fuerte en disputas.
- **El nombre de tu producto puede ser registrado como marca.** No es necesario para el lanzamiento, pero consideralo si el producto despega. Solicitud de marca en EE.UU.: $250-350 por clase.
- **Las licencias de codigo abierto en tus dependencias importan.** Si usas codigo con licencia MIT, estas bien. Si usas codigo con licencia GPL en un producto comercial, puedes necesitar hacer tu producto de codigo abierto. Verifica las licencias de tus dependencias antes de vender.

```bash
# Verifica las licencias de dependencias de tu proyecto (Node.js)
npx license-checker --summary

# Verifica licencias problematicas especificamente
npx license-checker --failOn "GPL-2.0;GPL-3.0;AGPL-3.0"

# Para proyectos Rust
cargo install cargo-license
cargo license
```

**7. Seguros**

No necesitas seguros para una biblioteca de componentes de $29. Si necesitas seguros si:
- Estas proporcionando servicios (consultoria, procesamiento de datos) donde los errores podrian causar perdidas a clientes
- Tu producto maneja datos sensibles (salud, financiero)
- Estas firmando contratos con clientes empresariales (te lo requeriran)

Cuando lo necesites, el seguro de responsabilidad profesional (errores y omisiones / E&O) cuesta $500-1,500/ano para un negocio de desarrollador solitario.

### Tu Turno

1. **Lee tu contrato de empleo.** Si estas empleado, encuentra la clausula de PI y la clausula de no competencia. Categorizalas: Seguro / Ambiguo / Restrictivo. Si es Ambiguo o Restrictivo, consulta a un abogado laboral antes de lanzar (muchos ofrecen consultas gratuitas de 30 minutos).

2. **Genera tus documentos legales.** Ve a Termly o Avodocs y genera una politica de privacidad y terminos de servicio para tu producto. Guardalos como HTML o Markdown. Despliegualos en `/privacy` y `/terms` en el dominio de tu producto.

3. **Toma tu decision de entidad.** Basandote en la orientacion anterior y tu residencia en {= regional.country | fallback("tu pais") =}, decide: lanzar como empresario individual (mas rapido) o formar una {= regional.business_entity_type | fallback("LLC/Ltd/equivalente") =} primero (mas proteccion). Anota tu decision y linea de tiempo.

4. **Verifica tus dependencias.** Ejecuta el verificador de licencias en tu proyecto. Resuelve cualquier dependencia GPL/AGPL antes de vender un producto comercial.

---

## Leccion 5: Canales de Distribucion Que Funcionan en 2026

*"Construirlo es el 20% del trabajo. Ponerlo frente a la gente es el otro 80%."*

### La Realidad de la Distribucion

La mayoria de los productos para desarrolladores fracasan no porque sean malos, sino porque nadie sabe que existen. La distribucion — poner tu producto frente a clientes potenciales — es la habilidad en la que la mayoria de los desarrolladores son mas debiles. Y es la habilidad que mas importa.

Aqui hay siete canales de distribucion clasificados por esfuerzo, linea de tiempo y retorno esperado. No necesitas los siete. Elige 2-3 que coincidan con tus fortalezas y tu audiencia.

### Canal 1: Hacker News

**Esfuerzo:** Alto | **Linea de tiempo:** Instantaneo (0-48 horas) | **Naturaleza:** Todo-o-nada

Hacker News (https://news.ycombinator.com) es el canal de distribucion de evento unico con mayor apalancamiento para productos de desarrolladores. Una publicacion Show HN en portada puede enviar 5,000-30,000 visitantes en 24 horas. Pero es impredecible — la mayoria de las publicaciones obtienen cero traccion.

**Lo que funciona en HN:**
- Productos tecnicos con detalles de implementacion interesantes
- Herramientas enfocadas en privacidad (la audiencia de HN se preocupa profundamente por la privacidad)
- Herramientas de codigo abierto con un nivel de pago
- Soluciones novedosas a problemas conocidos
- Productos con demos en vivo

**Lo que no funciona en HN:**
- Lanzamientos con mucho marketing ("Revolucionario AI-powered...")
- Productos que son wrappers de otros productos sin valor original
- Cualquier cosa que se sienta como un anuncio

**El Manual de Show HN:**

```
ANTES DE PUBLICAR:
1. Estudia publicaciones Show HN exitosas recientes en tu categoria
   https://hn.algolia.com — filtra por "Show HN", ordena por puntos
2. Prepara el titulo de tu publicacion: "Show HN: [Nombre] – [lo que hace, <70 caracteres]"
   Bueno: "Show HN: ScrubLog – Strip PII from Log Files in One Command"
   Malo: "Show HN: Introducing ScrubLog, the AI-Powered Log Anonymization Platform"
3. Ten una demo en vivo lista (los lectores de HN quieren probar, no leer)
4. Prepara respuestas a preguntas probables (decisiones tecnicas, razon de precios)

PUBLICANDO:
5. Publica entre las 7-9 AM hora del este de EE.UU., de martes a jueves
   (mayor trafico, mayor probabilidad de traccion)
6. El cuerpo de tu publicacion debe ser 4-6 parrafos:
   - Que es (1 parrafo)
   - Por que lo construiste (1 parrafo)
   - Detalles tecnicos (1-2 parrafos)
   - Que buscas (retroalimentacion, preguntas especificas)

DESPUES DE PUBLICAR:
7. Quedate en linea por 4 horas despues de publicar. Responde a CADA comentario.
8. Se humilde y tecnico. HN recompensa la honestidad sobre las limitaciones.
9. Si alguien encuentra un bug, arreglalo en vivo y responde "Arreglado, gracias."
10. No pidas a amigos que voten. HN tiene deteccion de anillos de votos.
```

**Resultados esperados (realistas):**
- 70% de las publicaciones Show HN: <10 puntos, <500 visitantes
- 20% de las publicaciones Show HN: 10-50 puntos, 500-3,000 visitantes
- 10% de las publicaciones Show HN: 50+ puntos, 3,000-30,000 visitantes

Es una loteria con probabilidades cargadas por el esfuerzo. Un gran producto con una gran publicacion tiene quizas un 30% de probabilidad de traccion significativa. No garantizado. Pero la ventaja es enorme.

### Canal 2: Reddit

**Esfuerzo:** Medio | **Linea de tiempo:** 1-7 dias | **Naturaleza:** Sostenible, repetible

Reddit es el canal de distribucion mas consistente para productos de desarrolladores. A diferencia de HN (un tiro), Reddit tiene cientos de subreddits de nicho donde tu producto es relevante.

**Seleccion de subreddit:**

```
Subreddits generales de desarrolladores:
- r/SideProject (140K+ miembros) — hecho para esto
- r/webdev (2.4M miembros) — enorme, competitivo
- r/programming (6.3M miembros) — muy competitivo, enfocado en noticias
- r/selfhosted (400K+ miembros) — si tu producto es autoalojable

Especificos de framework/lenguaje:
- r/reactjs, r/nextjs, r/sveltejs, r/vuejs — para herramientas frontend
- r/rust, r/golang, r/python — para herramientas especificas de lenguaje
- r/node — para herramientas y paquetes de Node.js

Especificos de dominio:
- r/devops — para herramientas de infraestructura/despliegue
- r/machinelearning — para herramientas de AI/ML
- r/datascience — para herramientas de datos
- r/sysadmin — para herramientas de administracion/monitoreo

La cola larga:
- Busca subreddits relacionados con tu nicho especifico
- Los subreddits mas pequenos (10K-50K miembros) a menudo tienen mejores
  tasas de conversion que los enormes
```

**Reglas de interaccion en Reddit:**

1. **Ten un historial real en Reddit** antes de publicar tu producto. Las cuentas que solo publican autopromocion se marcan y se les aplica shadowban.
2. **Sigue las reglas de cada subreddit** sobre autopromocion. La mayoria lo permite siempre que seas un miembro contribuyente.
3. **Interactua genuinamente.** Responde preguntas, aporta valor, se util en comentarios de otras publicaciones. Luego comparte tu producto.
4. **Publica a diferentes horas** para diferentes subreddits. Revisa https://later.com/reddit o herramientas similares para horarios de maxima actividad.

**Resultados esperados (realistas):**
- Publicacion en r/SideProject: 20-100 votos, 200-2,000 visitantes
- Subreddit de nicho (50K miembros): 10-50 votos, 100-1,000 visitantes
- Portada de r/webdev: 100-500 votos, 2,000-10,000 visitantes

### Canal 3: Twitter/X

**Esfuerzo:** Medio | **Linea de tiempo:** 2-4 semanas para ganar impulso | **Naturaleza:** Se compone con el tiempo

Twitter es un canal de construccion lenta. Tu primer tweet de lanzamiento obtendra 5 likes de tus amigos. Pero si compartes tu proceso de construccion consistentemente, tu audiencia se compone.

**La Estrategia de Construir en Publico:**

```
Semana 1: Empieza a compartir tu proceso de construccion (antes del lanzamiento)
- "Working on a [tipo de producto]. Here's the problem I'm solving: [captura]"
- "Day 3 of building [producto]. Got [funcionalidad] working: [GIF/captura]"

Semana 2: Comparte insights tecnicos de la construccion
- "TIL you need to [leccion tecnica] when building [tipo de producto]"
- "Architecture decision: chose [X] over [Y] because [razon]"

Semana 3: Lanzamiento
- Hilo de lanzamiento (formato de la Leccion 1)
- Comparte metricas especificas: "Day 1: X visitors, Y signups"

Semana 4+: Continuo
- Comparte retroalimentacion de clientes (con permiso)
- Comparte hitos de ingresos (a la gente le encantan los numeros reales)
- Comparte desafios y como los resolviste
```

**Con quien interactuar:**
- Sigue e interactua con desarrolladores en tu nicho
- Responde a tweets de cuentas mas grandes con comentarios reflexivos (no autopromocion)
- Unete a Twitter Spaces sobre tu area tematica
- Cita tweets de discusiones relevantes con tu perspectiva

**Resultados esperados (realistas):**
- 0-500 seguidores: Tweets de lanzamiento obtienen 5-20 likes, <100 visitantes
- 500-2,000 seguidores: Tweets de lanzamiento obtienen 20-100 likes, 100-500 visitantes
- 2,000-10,000 seguidores: Tweets de lanzamiento obtienen 100-500 likes, 500-5,000 visitantes

Twitter es una inversion de 6 meses, no una estrategia del dia de lanzamiento. Empieza ahora, incluso antes de que tu producto este listo.

### Canal 4: Product Hunt

**Esfuerzo:** Alto | **Linea de tiempo:** 1 dia de actividad intensa | **Naturaleza:** Impulso unico

Product Hunt (https://producthunt.com) es una plataforma de lanzamiento dedicada. Un top-5 diario puede enviar 3,000-15,000 visitantes. Pero requiere preparacion.

**Lista de Verificacion para Lanzamiento en Product Hunt:**

```
2 SEMANAS ANTES:
- [ ] Crea un perfil de maker en Product Hunt
- [ ] Construye tu listing en PH: eslogan, descripcion, imagenes, video
- [ ] Prepara 4-5 capturas de pantalla/GIFs de alta calidad
- [ ] Escribe un "primer comentario" que explique tu motivacion
- [ ] Alinea 10-20 personas para apoyar el dia del lanzamiento (no votos falsos —
      personas reales que probaran el producto y dejaran comentarios genuinos)
- [ ] Encuentra un "hunter" (alguien con gran seguimiento en PH para enviar tu producto)
      o envialo tu mismo

DIA DE LANZAMIENTO (12:01 AM hora del Pacifico):
- [ ] Estate en linea desde medianoche PT. PH se reinicia a medianoche.
- [ ] Publica tu "primer comentario" inmediatamente
- [ ] Comparte el enlace de PH en Twitter, LinkedIn, email, Discord
- [ ] Responde a CADA comentario en tu listing de PH
- [ ] Publica actualizaciones durante todo el dia ("Acabo de lanzar una correccion para [X]!")
- [ ] Monitorea todo el dia hasta medianoche PT

DESPUES:
- [ ] Agradece a todos los que apoyaron
- [ ] Escribe una publicacion de "lecciones aprendidas" (buen contenido para Twitter/blog)
- [ ] Incrusta la insignia de PH en tu landing page (prueba social)
```

> **Error Comun:** Lanzar en Product Hunt antes de que tu producto este listo. PH te da un solo disparo. Una vez que lanzas un producto, no puedes relanzarlo. Espera hasta que tu producto este pulido, tu landing page convierta, y tu flujo de pagos funcione. PH deberia ser tu "gran lanzamiento" — no tu lanzamiento suave.

**Resultados esperados (realistas):**
- Top 5 diario: 3,000-15,000 visitantes, 50-200 votos
- Top 10 diario: 1,000-5,000 visitantes, 20-50 votos
- Debajo del top 10: <1,000 visitantes. Impacto duradero minimo.

### Canal 5: Dev.to / Hashnode / Publicaciones Tecnicas en Blog

**Esfuerzo:** Bajo-medio | **Linea de tiempo:** Resultados SEO en 1-3 meses | **Naturaleza:** Cola larga, se compone para siempre

Escribe publicaciones tecnicas de blog que resuelvan problemas relacionados con tu producto, y menciona tu producto como la solucion.

**Estrategia de contenido:**

```
Para cada producto, escribe 3-5 publicaciones de blog:

1. "Como [resolver el problema que resuelve tu producto] en 2026"
   - Ensena el enfoque manual, luego menciona tu producto como el atajo

2. "Construi [producto] en 48 horas — esto es lo que aprendi"
   - Contenido de construccion en publico. Detalles tecnicos + reflexion honesta.

3. "[Competidor] vs [Tu Producto]: Comparacion Honesta"
   - Se genuinamente justo. Menciona donde el competidor gana.
   - Esto captura trafico de busqueda de comparacion de compras.

4. "[Concepto tecnico relacionado con tu producto] explicado"
   - Educacion pura. Menciona tu producto una vez al final.

5. "Las herramientas que uso para [el dominio de tu producto] en 2026"
   - Formato de lista. Incluye tu producto junto con otros.
```

**Donde publicar:**
- **Dev.to** (https://dev.to) — Gran audiencia de desarrolladores, buen SEO, gratis
- **Hashnode** (https://hashnode.com) — Buen SEO, opcion de dominio personalizado, gratis
- **Tu propio blog** — Mejor para SEO a largo plazo, el contenido es tuyo
- **Publica cruzado en todos lados.** Escribe una vez, publica en los tres. Usa URLs canonicas para evitar penalizaciones SEO.

**Resultados esperados por publicacion:**
- Dia 1: 100-1,000 vistas (distribucion de la plataforma)
- Mes 1-3: 50-200 vistas/mes (trafico de busqueda construyendose)
- Mes 6+: 100-500 vistas/mes (trafico de busqueda compuesto)

Una sola publicacion de blog bien escrita puede generar 200+ visitantes por mes durante anos. Cinco publicaciones generan 1,000+/mes. Esto se compone.

### Canal 6: Alcance Directo

**Esfuerzo:** Alto | **Linea de tiempo:** Inmediato | **Naturaleza:** Tasa de conversion mas alta

Los emails frios y DMs tienen la tasa de conversion mas alta de cualquier canal — pero tambien el mayor esfuerzo por lead. Usa esto para productos de mayor precio ($99+) o ventas B2B.

**Plantilla de email para alcanzar clientes potenciales:**

```
Asunto: Pregunta rapida sobre [su punto de dolor especifico]

Hola [nombre],

Vi tu [tweet/publicacion/comentario] sobre [problema especifico que mencionaron].

Construi [nombre del producto] especificamente para esto — [descripcion
de una oracion de lo que hace].

Estarias abierto a probarlo? Con gusto te doy acceso gratuito
a cambio de retroalimentacion.

[Tu nombre]
[Enlace al producto]
```

**Reglas para alcance frio:**
- Solo contacta a personas que hayan expresado publicamente el problema que tu producto resuelve
- Haz referencia a su publicacion/comentario especifico (prueba que no estas enviando emails masivos)
- Ofrece valor (acceso gratuito, descuento) en lugar de pedir dinero inmediatamente
- Mantenlo en menos de 5 oraciones
- Envia desde una direccion de email real (tu@tudominio.com, no gmail)
- Haz seguimiento una vez despues de 3-4 dias. Si no hay respuesta, detente.

**Resultados esperados:**
- Tasa de respuesta: 10-20% (email frio a destinatarios relevantes)
- Conversion de respuesta a prueba: 30-50%
- Conversion de prueba a pago: 20-40%
- Conversion efectiva: 1-4% de las personas contactadas se convierten en clientes

Para un producto de $99, contactar 100 personas = 1-4 ventas = $99-396. No es escalable, pero excelente para obtener clientes tempranos y retroalimentacion.

### Canal 7: SEO

**Esfuerzo:** Bajo continuo | **Linea de tiempo:** 3-6 meses para resultados | **Naturaleza:** Se compone para siempre

SEO es el mejor canal de distribucion a largo plazo. Es lento para empezar pero una vez que funciona, envia trafico gratuito indefinidamente.

**Estrategia SEO enfocada en desarrolladores:**

```
1. Apunta a palabras clave long-tail (mas faciles de rankear):
   En lugar de: "dashboard components"
   Apunta a: "tailwind dashboard components react typescript"

2. Crea una pagina por palabra clave:
   Cada publicacion de blog o pagina de docs apunta a una consulta de busqueda especifica

3. Implementacion tecnica:
   - Usa generacion de sitio estatico (Astro, Next.js SSG) para cargas rapidas
   - Agrega meta descripciones a cada pagina
   - Usa HTML semantico (jerarquia h1, h2, h3)
   - Agrega texto alt a cada imagen
   - Envía el sitemap a Google Search Console

4. Contenido que rankea para herramientas de desarrolladores:
   - Paginas de documentacion (sorprendentemente buenas para SEO)
   - Paginas de comparacion ("X vs Y")
   - Paginas de tutorial ("Como hacer X con Y")
   - Paginas de changelog (contenido fresco senala a Google)
```

```bash
# Envia tu sitemap a Google Search Console
# 1. Ve a https://search.google.com/search-console
# 2. Agrega tu propiedad (dominio o prefijo de URL)
# 3. Verifica la propiedad (registro DNS TXT o archivo HTML)
# 4. Envia la URL de tu sitemap: tudominio.com/sitemap.xml

# Si usas Astro:
pnpm add @astrojs/sitemap
# El sitemap se genera automaticamente en /sitemap.xml

# Si usas Next.js, agrega a next-sitemap.config.js:
# pnpm add next-sitemap
```

**Resultados esperados:**
- Mes 1-3: Trafico organico minimo (<100/mes)
- Mes 3-6: Trafico creciente (100-500/mes)
- Mes 6-12: Trafico significativo (500-5,000/mes)
- Mes 12+: Trafico compuesto que crece sin esfuerzo

{@ temporal market_timing @}

### Marco de Seleccion de Canales

No puedes hacer los siete bien. Elige 2-3 basandote en esta matriz:

| Si eres... | Prioriza | Salta |
|---|---|---|
| Lanzando este fin de semana | Reddit + HN | SEO, Twitter (demasiado lento) |
| Construyendo una audiencia primero | Twitter + Publicaciones de blog | Alcance directo, PH |
| Vendiendo un producto de $99+ | Alcance directo + HN | Dev.to (la audiencia espera gratis) |
| Jugando a largo plazo | SEO + Publicaciones de blog + Twitter | PH (un solo disparo, usalo despues) |
| No angloparlante | Dev.to + Reddit (global) | HN (centrado en EE.UU.) |

### Tu Turno

1. **Elige tus 2-3 canales.** Basandote en la matriz anterior y tu tipo de producto, elige los canales en los que te enfocaras. Anotalos con tu linea de tiempo planificada para cada uno.

2. **Escribe tu publicacion de Reddit.** Usando la plantilla de la Leccion 1, escribe tu borrador de publicacion para r/SideProject ahora mismo. Guardala. La publicaras el dia del lanzamiento.

3. **Escribe tu primera publicacion de blog.** Redacta una publicacion de "Como [resolver el problema que resuelve tu producto]". Esto va en Dev.to o tu blog dentro de la primera semana de lanzamiento. Apunta a 1,500-2,000 palabras.

4. **Configura Google Search Console.** Esto toma 5 minutos y te da datos SEO desde el dia uno. Hazlo antes de lanzar para tener datos de linea base.

---

## Leccion 6: Tu Lista de Verificacion de Lanzamiento

*"La esperanza no es una estrategia de lanzamiento. Las listas de verificacion si."*

### La Lista de Verificacion Pre-Lanzamiento

Revisa cada item. No lances hasta que cada item "Requerido" este marcado. Los items "Recomendados" pueden hacerse en la Semana 1 si es necesario.

**Producto (Requerido):**

```
- [ ] La funcionalidad central funciona como se describe en la landing page
- [ ] Sin bugs criticos en el flujo de compra -> entrega
- [ ] Funciona en Chrome, Firefox y Safari (para productos web)
- [ ] Landing page responsiva para movil (50%+ del trafico es movil)
- [ ] Los mensajes de error son utiles, no stack traces
- [ ] Estados de carga para cualquier operacion asincrona
```

**Landing Page (Requerido):**

```
- [ ] Titular claro: que hace en 8 palabras o menos
- [ ] Declaracion del problema: 3 puntos de dolor en lenguaje del cliente
- [ ] Seccion de solucion: capturas de pantalla o demos del producto
- [ ] Precios: visibles, claros, con boton de compra
- [ ] Llamada a la accion: un boton principal, visible sin hacer scroll
- [ ] Politica de privacidad enlazada en el pie de pagina
- [ ] Terminos de servicio enlazados en el pie de pagina
```

**Pagos (Requerido):**

```
- [ ] Flujo de checkout probado de principio a fin en modo prueba
- [ ] Flujo de checkout probado de principio a fin en modo real (compra de prueba de $1)
- [ ] El webhook recibe confirmacion de pago
- [ ] El cliente recibe acceso al producto despues del pago
- [ ] Proceso de devolucion documentado (RECIBIRAS solicitudes de devolucion)
- [ ] Recibo/factura enviado automaticamente
```

**Infraestructura (Requerido):**

```
- [ ] Dominio personalizado apuntando al sitio en vivo
- [ ] HTTPS funcionando (candado verde)
- [ ] Monitoreo de uptime activo
- [ ] Script de analiticas instalado y recibiendo datos
- [ ] Email de contacto funcionando (tu@tudominio.com)
```

**Distribucion (Requerido):**

```
- [ ] Publicacion de Reddit redactada y lista
- [ ] Publicacion Show HN redactada y lista (si aplica)
- [ ] Hilo de lanzamiento en Twitter redactado
- [ ] 2-3 comunidades identificadas para compartir
```

**Recomendado (Semana 1):**

```
- [ ] Meta tags OpenGraph para previews de compartir en redes sociales
- [ ] Pagina 404 personalizada
- [ ] Pagina o seccion de FAQ
- [ ] Secuencia de emails de onboarding de clientes (bienvenida + primeros pasos)
- [ ] Pagina de changelog (incluso si esta vacia — muestra compromiso con las actualizaciones)
- [ ] Publicacion de blog: "Construi [producto] en 48 horas"
- [ ] Google Search Console verificado y sitemap enviado
```

### Items de Accion Post-Lanzamiento

**Dia 1 (Dia de Lanzamiento):**

```
Manana:
- [ ] Publica en Reddit (r/SideProject + 1 subreddit de nicho)
- [ ] Publica Show HN (si aplica)
- [ ] Publica hilo de lanzamiento en Twitter

Todo el dia:
- [ ] Responde a CADA comentario en Reddit, HN y Twitter
- [ ] Monitorea logs de error y analiticas en tiempo real
- [ ] Arregla cualquier bug descubierto por usuarios inmediatamente
- [ ] Envia email de agradecimiento personal a cada cliente

Noche:
- [ ] Revisa metricas: visitantes, tasa de conversion, ingresos
- [ ] Captura de pantalla de tu dashboard de analiticas (querras esto despues)
- [ ] Anota las 3 piezas de retroalimentacion mas comunes
```

**Semana 1:**

```
- [ ] Responde a toda la retroalimentacion y solicitudes de soporte dentro de 24 horas
- [ ] Arregla los 3 bugs/problemas principales identificados durante el lanzamiento
- [ ] Escribe y publica tu primera publicacion de blog
- [ ] Envia un email de seguimiento a todos los clientes pidiendo retroalimentacion
- [ ] Revisa analiticas: cuales paginas tienen las tasas de rebote mas altas?
- [ ] Configura un metodo simple de recopilacion de retroalimentacion (email, Typeform o Canny)

Metricas semanales para registrar:
| Metrica                | Objetivo  | Real   |
|------------------------|-----------|--------|
| Visitantes unicos      | 500+      |        |
| Tasa de clic al checkout | 2-5%   |        |
| Conversion de compra   | 1-3%      |        |
| Ingresos               | $50+      |        |
| Solicitudes de soporte | <10       |        |
| Solicitudes de devolucion | <2     |        |
```

**Mes 1:**

```
- [ ] Lanza 4 mejoras semanales basadas en retroalimentacion de clientes
- [ ] Publica 2+ publicaciones de blog (construyendo SEO)
- [ ] Recopila 3+ testimonios de clientes
- [ ] Agrega testimonios a la landing page
- [ ] Evalua precios: demasiado altos? demasiado bajos? (revisa datos de conversion)
- [ ] Planifica tu "gran lanzamiento" en Product Hunt (si aplica)
- [ ] Empieza a construir lista de email para futuros lanzamientos de productos
- [ ] Revisa y ajusta tu estrategia de canales de distribucion

Revision financiera mensual:
| Categoria                 | Monto     |
|---------------------------|-----------|
| Ingresos brutos           | $         |
| Tarifas del procesador de pagos | $  |
| Costos de hosting/infra   | $         |
| Costos de API             | $         |
| Ganancia neta             | $         |
| Horas invertidas          |           |
| Tasa por hora efectiva    | $         |
```

### El Dashboard de Metricas

Configura un dashboard simple de metricas que revises diariamente. No necesita ser sofisticado — una hoja de calculo funciona.

```
=== METRICAS DIARIAS (revisa cada manana) ===

Fecha: ___
Visitantes ayer: ___
Nuevos clientes ayer: ___
Ingresos ayer: $___
Solicitudes de soporte: ___
Uptime: ___%

=== METRICAS SEMANALES (revisa cada lunes) ===

Semana del: ___
Total de visitantes: ___
Total de clientes: ___
Total de ingresos: $___
Tasa de conversion: ___% (clientes / visitantes)
Pagina mas visitada: ___
Principal fuente de trafico: ___
Principal tema de retroalimentacion: ___

=== METRICAS MENSUALES (revisa el 1ro del mes) ===

Mes: ___
Total de ingresos: $___
Total de gastos: $___
Ganancia neta: $___
Total de clientes: ___
Devoluciones: ___
Tasa de desercion (suscripciones): ___%
MRR (Ingresos Mensuales Recurrentes): $___
Tasa de crecimiento vs. mes anterior: ___%
```

**Configuracion de analiticas respetuosas con la privacidad:**

```javascript
// Si usas Plausible, obtienes la mayoria de esto en su dashboard.
// Para rastreo de eventos personalizados:

// Rastrear clics al checkout
document.querySelector('#buy-button').addEventListener('click', () => {
  plausible('Checkout Click', {
    props: { tier: 'pro', price: '59' }
  });
});

// Rastrear compras exitosas (llama desde tu manejador de webhook exitoso)
plausible('Purchase', {
  props: { tier: 'pro', revenue: '59' }
});
```

### Cuando Doblar la Apuesta, Pivotar o Matar

Despues de 30 dias de datos, tienes suficiente senal para tomar una decision:

**Doblar la Apuesta (sigue adelante, invierte mas):**

```
Senales:
- Los ingresos crecen semana tras semana (incluso si lentamente)
- Los clientes proporcionan solicitudes de funcionalidades especificas (quieren MAS)
- La tasa de conversion es estable o mejorando
- Estas obteniendo trafico organico (gente encontrandote sin tus publicaciones)
- Al menos un cliente dijo "esto me ahorro [tiempo/dinero]"

Acciones:
- Aumenta los esfuerzos de distribucion (agrega un canal)
- Lanza la funcionalidad mas solicitada
- Sube los precios ligeramente
- Empieza a construir una lista de email para futuros lanzamientos
```

**Pivotar (cambia el angulo, mantiene el nucleo):**

```
Senales:
- Visitantes pero sin ventas (la gente esta interesada pero no compra)
- Ventas de audiencia inesperada (personas diferentes a las que apuntaste)
- Los clientes usan el producto diferente de lo que esperabas
- La retroalimentacion consistentemente apunta a un problema diferente del que estas resolviendo

Acciones:
- Reescribe la landing page para la audiencia/caso de uso real
- Ajusta los precios basandote en la disposicion a pagar de la audiencia real
- Reprioriza funcionalidades hacia lo que la gente realmente usa
- Conserva el codigo, cambia el posicionamiento
```

**Matar (detente, aprende, construye algo mas):**

```
Senales:
- Sin visitantes a pesar de esfuerzos de distribucion (problema de demanda)
- Visitantes pero cero clics al checkout (problema de posicionamiento/precio
  que persiste despues de ajustes)
- Ingresos estancados por 4+ semanas sin tendencia de crecimiento
- Te da pavor trabajar en ello (la motivacion importa para productos solitarios)
- El mercado ha cambiado (un competidor lanzo, la tecnologia cambio)

Acciones:
- Escribe un post-mortem: que funciono, que no, que aprendiste
- Guarda el codigo — piezas podrian ser utiles en tu proximo producto
- Toma una semana libre de construir
- Inicia el proceso de validacion para una nueva idea
- Esto no es un fracaso. Son datos. La mayoria de los productos no funcionan.
  Los desarrolladores que ganan dinero son los que lanzan 5 productos,
  no los que pasan un ano en uno.
```

### La Plantilla del Documento de Lanzamiento

Este es tu entregable para el Modulo E. Crea este documento y completalo mientras ejecutas tu lanzamiento.

```markdown
# Documento de Lanzamiento: [Nombre del Producto]

## Pre-Lanzamiento

### Resumen de Validacion
- **Volumen de busqueda:** [numeros de Google Trends/Ahrefs]
- **Evidencia de hilos:** [enlaces a 5+ hilos mostrando demanda]
- **Auditoria de competidores:** [3+ competidores con fortalezas/debilidades]
- **Evidencia de "10 personas pagarian":** [como lo validaste]

### Producto
- **URL:** [URL del producto en vivo]
- **Dominio:** [dominio comprado]
- **Hosting:** [plataforma]
- **Funcionalidades centrales (v0.1):**
  1. [Funcionalidad 1]
  2. [Funcionalidad 2]
  3. [Funcionalidad 3]

### Precios
- **Precio:** $[monto]
- **Estructura de niveles:** [Basico/Pro/Equipo o nivel unico]
- **Plataforma de pago:** [Lemon Squeezy/Stripe]
- **URL de checkout:** [enlace]

### Legal
- **Politica de privacidad:** [URL]
- **Terminos de servicio:** [URL]
- **Entidad comercial:** [tipo o "empresario individual"]

## Lanzamiento

### Canales de Distribucion
| Canal   | URL de la Publicacion | Fecha de Publicacion | Resultados |
|---------|----------------------|---------------------|------------|
| Reddit  | [enlace]             | [fecha]             | [visitantes, votos] |
| HN      | [enlace]             | [fecha]             | [visitantes, puntos] |
| Twitter | [enlace]             | [fecha]             | [impresiones, clics] |

### Metricas del Dia 1
- Visitantes: ___
- Clics al checkout: ___
- Compras: ___
- Ingresos: $___

### Metricas de la Semana 1
- Total de visitantes: ___
- Total de compras: ___
- Total de ingresos: $___
- Tasa de conversion: ___%
- Principal retroalimentacion: ___

### Metricas del Mes 1
- Total de ingresos: $___
- Total de gastos: $___
- Ganancia neta: $___
- Total de clientes: ___
- Decision: [ ] Doblar la apuesta [ ] Pivotar [ ] Matar

## Hoja de Ruta Post-Lanzamiento
- Semana 2: [mejora planificada]
- Semana 3: [mejora planificada]
- Semana 4: [mejora planificada]
- Mes 2: [funcionalidad/expansion planificada]

## Lecciones Aprendidas
- Lo que funciono: ___
- Lo que no funciono: ___
- Lo que haria diferente: ___
```

### Integracion 4DA

> **Integracion 4DA:** Las senales accionables de 4DA clasifican contenido por urgencia. Una senal "critica" sobre una vulnerabilidad en un paquete popular significa: construye la correccion o herramienta de migracion AHORA, antes que nadie. Una senal de "tendencia ascendente" sobre un nuevo framework significa: construye el kit de inicio este fin de semana mientras la competencia es casi nula. El sprint de 48 horas de la Leccion 1 funciona mejor cuando tu idea viene de una senal sensible al tiempo. Conecta tu feed de inteligencia 4DA a tu calendario de sprints — cuando aparezca una oportunidad de alta urgencia, bloquea el proximo fin de semana y ejecuta. La diferencia entre los desarrolladores que capturan oportunidades y los que las pierden no es el talento. Es la velocidad. 4DA te da el radar. Este modulo te da la secuencia de lanzamiento. Juntos, convierten senales en ingresos.

### Tu Turno

1. **Completa la lista de verificacion pre-lanzamiento.** Revisa cada item. Marca cada uno como hecho o programa cuando lo haras. No te saltes los items "Requeridos".

2. **Crea tu Documento de Lanzamiento.** Copia la plantilla anterior en tu herramienta de documentos preferida. Completa todo lo que sabes ahora. Deja espacios en blanco para las metricas que llenaras durante y despues del lanzamiento.

3. **Establece tu fecha de lanzamiento.** Abre tu calendario. Elige un sabado especifico dentro de las proximas 2 semanas. Anotalo. Dile a alguien — un amigo, una pareja, un seguidor de Twitter. La rendicion de cuentas lo hace real.

4. **Establece tus criterios de abandono.** Antes de lanzar, decide: "Si tengo menos de [X] ventas despues de 30 dias a pesar de [Y] esfuerzo de distribucion, [pivoteare/abandonare]." Escribe esto en tu Documento de Lanzamiento. Tener criterios pre-comprometidos evita que inviertas meses en un producto muerto por la falacia del costo hundido.
{? if progress.completed("S") ?}
   Consulta tu Documento de Stack Soberano del Modulo S — tus restricciones de presupuesto y costos operativos definen lo que "rentable" significa para tu situacion especifica.
{? endif ?}

5. **Lanzalo.** Tienes el manual. Tienes las herramientas. Tienes el conocimiento. Lo unico que queda es el acto. Internet esta esperando.

---

## Modulo E: Completo

### Lo Que Has Construido en Dos Semanas

{? if dna.identity_summary ?}
> **Tu identidad como desarrollador:** {= dna.identity_summary | fallback("Aun no perfilada") =}. Todo lo que construiste en este modulo aprovecha esta identidad — tu velocidad de lanzamiento es una funcion de tu experiencia existente.
{? endif ?}

Mira lo que ahora tienes que no tenias cuando empezaste este modulo:

1. **Un marco de ejecucion de 48 horas** que puedes repetir para cada producto que construyas — de idea validada a producto en vivo en un fin de semana.
2. **Una mentalidad de lanzamiento** que prioriza la existencia sobre la perfeccion, los datos sobre las suposiciones, y la iteracion sobre la planificacion.
3. **Una estrategia de precios** fundamentada en psicologia real y numeros reales, no esperanza y cobrar de menos.
4. **Una base legal** que te protege sin paralizarte — politica de privacidad, terminos, plan de entidad.
5. **Un manual de distribucion** con plantillas especificas, timing y resultados esperados para siete canales.
6. **Una lista de verificacion de lanzamiento y sistema de seguimiento** que convierte el caos en proceso — repetible, medible, mejorable.
7. **Un producto en vivo, aceptando pagos, con humanos reales visitandolo.**

Ese ultimo es el que importa. Todo lo demas es preparacion. El producto es la prueba.

### Lo Que Viene: Modulo E2 — Ventaja en Evolucion

El Modulo E1 te llevo al lanzamiento. El Modulo E2 te mantiene adelante.

Esto es lo que cubre el Modulo E2:

- **Sistemas de deteccion de tendencias** — como detectar oportunidades 2-4 semanas antes de que sean obvias
- **Monitoreo competitivo** — rastrear lo que otros en tu espacio estan construyendo y cobrando
- **Surfear olas tecnologicas** — cuando adoptar nueva tecnologia en tus productos y cuando esperar
- **Desarrollo de clientes** — convertir tus primeros 10 clientes en tu consejo asesor de producto
- **La decision del segundo producto** — cuando construir el producto #2 vs. mejorar el producto #1

Los desarrolladores que generan ingresos consistentes no son los que lanzan una vez. Son los que lanzan, iteran y se mantienen adelante del mercado. El Modulo E2 te da el sistema para mantenerte adelante.

### La Hoja de Ruta Completa de STREETS

| Modulo | Titulo | Enfoque | Duracion |
|--------|--------|---------|----------|
| **S** | Configuracion Soberana | Infraestructura, legal, presupuesto | Semanas 1-2 |
| **T** | Fosos Tecnicos | Ventajas defensibles, activos propietarios | Semanas 3-4 |
| **R** | Motores de Ingresos | Manuales de monetizacion especificos con codigo | Semanas 5-8 |
| **E** | Manual de Ejecucion | Secuencias de lanzamiento, precios, primeros clientes | Semanas 9-10 (completo) |
| **E** | Ventaja en Evolucion | Mantenerse adelante, deteccion de tendencias, adaptacion | Semanas 11-12 |
| **T** | Automatizacion Tactica | Automatizar operaciones para ingresos pasivos | Semanas 13-14 |
| **S** | Apilando Fuentes | Multiples fuentes de ingresos, estrategia de portafolio | Semanas 15-16 |

Has pasado el punto medio. Tienes un producto en vivo. Eso te pone adelante del 95% de los desarrolladores que quieren construir ingresos independientes pero nunca llegan tan lejos.

> **Progreso STREETS:** {= progress.completed_count | fallback("0") =} de {= progress.total_count | fallback("7") =} modulos completados. {? if progress.completed_modules ?}Completados: {= progress.completed_modules | fallback("Ninguno aun") =}.{? endif ?}

Ahora hazlo crecer.

---

**Tu producto esta en vivo. Tu checkout funciona. Los humanos pueden pagarte dinero.**

**Todo despues de esto es optimizacion. Y la optimizacion es la parte divertida.**

*Tu equipo. Tus reglas. Tus ingresos.*
