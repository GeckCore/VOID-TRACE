# 🔍 AI-Ghost-Hunter

Herramienta de análisis de código que detecta si un repositorio fue escrito por una IA (ChatGPT, Claude, Copilot…) usando análisis de árbol sintáctico (AST) y estilometría.

```
╔══════════════════════════════════════════════════════╗
║  REPOSITORY AI-SCORE   87.3%                         ║
║  ████████████████████████░░░░░░░░░░░░░░░░░░░░░░      ║
║  ◉ DEFINITE AI GENERATION                            ║
╚══════════════════════════════════════════════════════╝

  FILE                                    AI-SCORE  LANG   LOC
  ──────────────────────────────────────────────────────────────
  src/services/authenticationService.js    94.2%     JS    312
  src/api/userController.ts                91.7%     TS    198
  utils/dataProcessor.py                   88.3%     Py    441
```

---

## Instalación en Windows (paso a paso)

> **No necesitas saber programar.** Sigue los pasos en orden. Si algo falla, lee la sección [Solución de errores comunes](#solución-de-errores-comunes).

---

### Paso 1 — Instalar Rust

Rust es el lenguaje en el que está escrita la herramienta. Necesitas instalarlo una sola vez.

1. Abre tu navegador y ve a: **https://rustup.rs**
2. Haz clic en el botón de descarga **"64-bit"** (o 32-bit si tu PC es antiguo)
3. Se descargará un archivo llamado `rustup-init.exe`
4. Ejecútalo haciendo doble clic
5. Aparecerá una ventana negra. Pulsa **`1`** y luego **`Enter`** para la instalación estándar
6. Espera a que termine (tarda 2-5 minutos)
7. Cuando diga `Rust is installed now. Great!`, pulsa **Enter** para cerrar

---

### Paso 2 — Instalar Visual C++ Build Tools

Rust necesita un compilador de C para construir las gramáticas de análisis de código.

1. Ve a: **https://visualstudio.microsoft.com/visual-cpp-build-tools/**
2. Haz clic en **"Download Build Tools"**
3. Ejecuta el instalador descargado (`vs_BuildTools.exe`)
4. En la pantalla de selección, marca la casilla **"Desarrollo para el escritorio con C++"**
5. Haz clic en **"Instalar"** (descarga ~2-3 GB, tarda varios minutos)
6. Reinicia el PC cuando termine

> **Alternativa más rápida:** Si ya tienes Visual Studio instalado (cualquier versión), esto ya está incluido.

---

### Paso 3 — Instalar Git

Git permite a la herramienta descargar repositorios de GitHub automáticamente.

1. Ve a: **https://git-scm.com/download/win**
2. Descarga el instalador (el botón grande de descarga)
3. Ejecútalo y pulsa **"Next"** en todas las pantallas (los valores por defecto están bien)
4. Haz clic en **"Install"** y luego en **"Finish"**

---

### Paso 4 — Descargar VOID-TRACE

Tienes dos opciones:

**Opción A — Descargar ZIP (más fácil):**

1. Ve a la página del repositorio en GitHub
2. Haz clic en el botón verde **"Code"**
3. Haz clic en **"Download ZIP"**
4. Extrae el ZIP en `C:\Users\TU_NOMBRE_DE_USUARIO\` — quedará una carpeta como `ai-ghost-hunter-main`
5. Renómbrala a `ai-ghost-hunter` si quieres (opcional)

**Opción B — Con Git:**

1. Abre **PowerShell** (búscalo en el menú inicio, escribe "powershell")
2. Escribe estos comandos uno por uno, pulsando Enter después de cada uno:

```powershell
cd C:\Users\TU_NOMBRE_DE_USUARIO
git clone https://github.com/GeckCore/VOID-TRACE
cd VOID-TRACE
```

> Sustituye `TU_NOMBRE_DE_USUARIO` por el que aparece en tu terminal: `PS C:\Users\mihoy>` → el tuyo es `mihoy`

---

### Paso 5 — Compilar la herramienta

> ⚠️ **MUY IMPORTANTE:** Cierra PowerShell completamente y ábrelo de nuevo antes de este paso. Así detecta Rust correctamente.

1. Abre **PowerShell** (menú inicio → escribe "powershell" → Enter)
2. Navega a la carpeta del proyecto:

```powershell
cd C:\Users\TU_NOMBRE_DE_USUARIO\VOID-TRACE
```

3. Comprueba que Rust está instalado correctamente:

```powershell
cargo --version
```

Debe mostrar algo como: `cargo 1.78.0 (xxxxxxx 2024-xx-xx)`

4. Compila la herramienta:

```powershell
cargo build --release
```

Verás mucho texto desfilar por pantalla — es normal, está descargando y compilando dependencias. **Tarda 3-8 minutos la primera vez.** Las siguientes veces es instantáneo.

Cuando termine verás algo como:

```
   Compiling ai-ghost-hunter v1.0.0
    Finished release [optimized] target(s) in 4m 32s
```

El programa ya está listo en: `target\release\aigh.exe`

---

### Paso 6 — Usar la herramienta

En PowerShell, desde dentro de la carpeta del proyecto:

```powershell
# Analizar un repositorio de GitHub (lo descarga automáticamente)
.\target\release\aigh.exe https://github.com/GeckCore/star-forensics

# Analizar una carpeta local de tu PC
.\target\release\aigh.exe C:\Users\TU_NOMBRE\alguna-carpeta

# Modo detallado (muestra estadísticas por archivo)
.\target\release\aigh.exe https://github.com/owner/repo --verbose

# Mostrar todos los archivos sin límite
.\target\release\aigh.exe https://github.com/owner/repo --all

# Salida en JSON (para scripts o automatización)
.\target\release\aigh.exe https://github.com/owner/repo --json
```

---

### Paso 7 — Hacerlo disponible desde cualquier carpeta (opcional)

Ahora mismo solo funciona si estás dentro de la carpeta del proyecto. Para usarlo desde cualquier lugar:

1. Crea una carpeta `C:\tools\` (puedes hacerlo desde el Explorador de archivos)
2. Copia `target\release\aigh.exe` a `C:\tools\aigh.exe`
3. Añade `C:\tools\` al PATH del sistema:
   - Pulsa `Win + R`, escribe `sysdm.cpl` y pulsa Enter
   - Ve a la pestaña **"Opciones avanzadas"**
   - Haz clic en **"Variables de entorno..."**
   - En la sección inferior **"Variables del sistema"**, busca `Path` y haz doble clic
   - Haz clic en **"Nuevo"** y escribe `C:\tools`
   - Pulsa **OK** en todas las ventanas abiertas
4. Cierra PowerShell y ábrelo de nuevo
5. Ahora puedes escribir desde cualquier carpeta:

```powershell
aigh https://github.com/GeckCore/star-forensics
```

---

## Solución de errores comunes

### ❌ `cargo : El término 'cargo' no se reconoce...`

**Causa:** PowerShell no detecta Rust porque estaba abierto durante la instalación, o Rust no terminó de instalarse.

**Solución:**
1. Cierra **todas** las ventanas de PowerShell
2. Ábrelo de nuevo desde el menú inicio
3. Escribe `cargo --version`
4. Si sigue fallando → reinicia el PC y vuelve a intentarlo
5. Si sigue fallando tras reiniciar → vuelve a ejecutar `rustup-init.exe` del Paso 1

---

### ❌ `chmod : El término 'chmod' no se reconoce...`

**Causa:** `chmod` es un comando de Linux/Mac. **No existe en Windows y no lo necesitas.**

**Solución:** Ignora ese comando completamente. Sigue con el paso siguiente.

---

### ❌ `sh : El término 'sh' no se reconoce...`

**Causa:** El instalador de Rust para Linux (`curl ... | sh`) no funciona en Windows.

**Solución:** Usa el instalador `.exe` de **https://rustup.rs** como se explica en el Paso 1. No copies comandos de Linux.

---

### ❌ `source ~/.cargo/env`

**Causa:** Comando de Linux. No funciona en PowerShell.

**Solución:** Cierra PowerShell y ábrelo de nuevo. Eso recarga el PATH automáticamente en Windows.

---

### ❌ `error: linker 'link.exe' not found`

**Causa:** Las Visual C++ Build Tools no están instaladas correctamente.

**Solución:**
1. Vuelve al **Paso 2** y asegúrate de marcar **"Desarrollo para el escritorio con C++"**
2. Reinicia el PC después de que instale
3. Vuelve a intentar `cargo build --release`

---

### ❌ El análisis de un repo de GitHub falla con error de red o de autenticación

**Causa:** Repositorio privado, o has hecho muchas peticiones y GitHub te limita temporalmente.

**Solución — Crear un token de GitHub:**

1. Ve a **https://github.com/settings/tokens** (debes estar logueado)
2. Haz clic en **"Generate new token"** → **"Generate new token (classic)"**
3. Ponle un nombre cualquiera (ej: `ghost-hunter`)
4. Marca la casilla **`repo`**
5. Haz clic en **"Generate token"** al final de la página
6. Copia el token (empieza por `ghp_`) — **guárdalo, solo se ve una vez**
7. Úsalo así:

```powershell
.\target\release\aigh.exe https://github.com/owner/repo --token ghp_XXXXXXXXXXXX
```

O para no escribirlo cada vez, guárdalo en la sesión de PowerShell:

```powershell
$env:GITHUB_TOKEN = "ghp_XXXXXXXXXXXX"
.\target\release\aigh.exe https://github.com/owner/repo
```

---

## Referencia rápida de comandos

```powershell
# Uso básico
.\target\release\aigh.exe <URL_de_GitHub_o_ruta_local> [opciones]

# Si lo instalaste en C:\tools\ y añadiste al PATH:
aigh <URL_o_ruta> [opciones]

# Opciones:
  --token <TOKEN>     Token de GitHub para repos privados o límite de API
  --min-size <BYTES>  Ignorar archivos menores a N bytes (default: 150)
  --verbose           Mostrar estadísticas detalladas por archivo
  --top <N>           Cuántos archivos mostrar en la tabla (default: 40)
  --all               Mostrar todos los archivos sin límite
  --json              Salida en formato JSON
```

---

## Qué detecta y cómo

La herramienta analiza el **árbol sintáctico real del código** (no busca palabras clave), midiendo 4 señales:

| Señal | Qué mide | Por qué delata a la IA |
|-------|----------|------------------------|
| **[N] Naming Entropy** | Predictibilidad de nombres de variables | Las IAs usan prefijos como `get_`, `handle_`, `validate_` de forma muy sistemática |
| **[C] Comment Predictability** | Patrones gramaticales en comentarios | Las IAs explican *qué* hace el código; los humanos explican *por qué* |
| **[B] Boilerplate Consistency** | Consistencia de formato y espaciado | Las IAs nunca dejan espacios al final de línea ni mezclan tabs y espacios |
| **[V] Complexity / Verbosity** | Líneas de código por punto de decisión | Las IAs escriben 18-35 líneas por rama; los humanos escriben 6-12 |

### Umbrales de puntuación

| Puntuación | Veredicto |
|------------|-----------|
| ≥ 85% | Generación IA definitiva |
| 70–85% | Alta probabilidad de IA |
| 55–70% | Señal moderada de IA |
| 40–55% | Ambiguo / señales mixtas |
| 25–40% | Probablemente humano |
| < 25% | Huella humana fuerte |

### Lenguajes soportados

`.rs` (Rust) · `.py` (Python) · `.js` / `.jsx` (JavaScript) · `.ts` / `.tsx` (TypeScript)

---

## Resumen visual del proceso en Windows

```
  1. Descarga rustup-init.exe   →  https://rustup.rs
             ↓
  2. Descarga Build Tools (C++) →  https://visualstudio.microsoft.com/visual-cpp-build-tools/
             ↓
  3. Descarga Git               →  https://git-scm.com/download/win
             ↓
  4. Reinicia el PC
             ↓
  5. Abre PowerShell → escribe:
       cd C:\Users\TU_NOMBRE\ai-ghost-hunter
             ↓
  6. cargo build --release      (espera 3-8 min, solo la primera vez)
             ↓
  7. .\target\release\aigh.exe https://github.com/owner/repo
```
