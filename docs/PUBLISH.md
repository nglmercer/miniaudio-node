# 🚀 Guía de Publicación Multiplataforma

## 📋 Estrategia Recomendada: GitHub Actions

**Usa GitHub Actions para compilación multiplataforma automática** - este es el mejor enfoque para módulos nativos.

### 1. Configurar GitHub Actions

Crear `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        
      - name: Build native module
        run: |
          cd native
          cargo build --release
          cargo test --release
          
      - name: Setup Bun (Linux/macOS)
        if: runner.os != 'Windows'
        run: |
          curl -fsSL https://bun.sh/install | bash
          echo "$HOME/.bun/bin" >> $GITHUB_PATH
          
      - name: Setup Bun (Windows)
        if: runner.os == 'Windows'
        run: |
          powershell -c "irm bun.sh/install.ps1 | iex"
          echo "$HOME/.bun/bin" >> $GITHUB_PATH
          
      - name: Run tests
        run: bun test
        
      - name: Publish to NPM
        env:
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: npm publish
```

### 2. Proceso de Lanzamiento

```bash
# 1. Actualizar versión
npm version patch  # o minor/major

# 2. Push tag
git push --tags

# 3. GitHub Actions hará:
#    - Build para Windows, macOS, Linux
#    - Ejecutar tests en cada plataforma
#    - Publicar en npm automáticamente
```

### 3. Variables de Entorno

Configurar en GitHub Secrets:
- `NPM_TOKEN`: Token de publicación de npm

## 🔄 Alternativa: Build Manual Multiplataforma

Si prefieres builds manuales:

```bash
# 1. Build para cada plataforma manualmente
# Windows (en Windows)
cd native && cargo build --release --target x86_64-pc-windows-msvc

# macOS (en macOS)  
cd native && cargo build --release --target x86_64-apple-darwin

# Linux (en Linux)
cd native && cargo build --release --target x86_64-unknown-linux-gnu

# 2. Publicar desde una plataforma
npm publish
```

## 📁 Estructura Actual Simplificada

```
miniaudio_node/
├── 🦀 native/                 # Módulo nativo Rust
│   ├── src/
│   │   └── lib.rs          # Implementación Rust FFI
│   ├── Cargo.toml           # Dependencias Rust
│   ├── index.js             # Entry point del módulo nativo
│   ├── package.json          # Configuración del paquete nativo
│   └── target/              # Artefactos de build
│
├── 🧪 tests/                 # Suite de tests
│   ├── unit/                # Tests unitarios
│   │   └── audio-player.test.ts
│   └── integration/         # Tests de integración
│       └── playback.test.ts
│
├── 📚 examples/               # Ejemplos de uso
│   ├── usage.js             # Ejemplo básico JavaScript
│   └── typescript/          # Ejemplos TypeScript
│       └── advanced.ts       # Ejemplo avanzado
│
├── 📖 docs/                  # Documentación
│   ├── CHANGELOG.md         # Historial de versiones
│   ├── LICENSE              # Licencia
│   └── PROJECT_STRUCTURE.md  # Estructura del proyecto
│
├── 📄 package.json            # Configuración del paquete
├── 🚫 .gitignore             # Reglas de git ignore
└── 📖 README.md               # Documentación principal
```

## 🛠️ Scripts Simplificados

| Script | Descripción |
|--------|-------------|
| `bun build` | Build módulo nativo Rust |
| `bun build:debug` | Build con símbolos de debug |
| `bun test` | Ejecutar todos los tests |
| `bun test:watch` | Tests en modo watch |
| `bun clean` | Limpiar artefactos de build |
| `bun dev` | Build y test |
| `bun lint` | Ejecutar ESLint |
| `bun format` | Formatear código con Prettier |

## 🎯 Consideraciones Multiplataforma

- **GitHub Actions** es recomendado para builds consistentes
- **Dependencias nativas** son específicas de plataforma
- **Testing** debe ejecutarse en todas las plataformas objetivo
- **Gestión de versiones** debe usar versionado semántico
- **Automatización de releases** previene errores humanos

## 📦 Archivos Incluidos en npm

```json
"files": [
  "native/",
  "README.md",
  "LICENSE",
  "CHANGELOG.md"
]
```

## 🔧 Configuración de Package.json

```json
{
  "main": "./native/index.js",
  "types": "./native/index.d.ts",
  "exports": {
    ".": {
      "import": "./native/index.js",
      "types": "./native/index.d.ts",
      "default": "./native/index.js"
    }
  }
}
```

## ✅ Verificación Final

Antes de publicar:

```bash
# 1. Ejecutar tests
bun test

# 2. Verificar build
bun run build

# 3. Limpiar
bun run clean

# 4. Publicar
npm publish
```

## 🎉 Resumen

La librería ahora está:
- ✅ **Simplificada** - Solo lo necesario
- ✅ **Testeada** - 38 tests pasando
- ✅ **Documentada** - README y docs actualizados
- ✅ **Lista para publicar** - Configuración multiplataforma lista

**Recomendación**: Usa GitHub Actions para publicación automática multiplataforma.
