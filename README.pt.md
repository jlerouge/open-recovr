<div align="center">

<img src="assets/icon.svg" alt="" width="96" height="96">

# Open Recovr

**Recupere seus arquivos excluídos, de forma simples.**

Uma só tela, sem assistente: escolha a unidade, busque e marque o que quer recuperar.<br>
Livre e gratuito, com o [PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec) como motor.

[English](README.md) · [Français](README.fr.md) · [Español](README.es.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · **Português (Brasil)** · [Nederlands](README.nl.md) · [Polski](README.pl.md) · [Українська](README.uk.md)

[![Tests](https://img.shields.io/github/actions/workflow/status/jlerouge/open-recovr/ci.yml?branch=main&label=testes&logo=github)](https://github.com/jlerouge/open-recovr/actions/workflows/ci.yml)
[![Versão](https://img.shields.io/github/v/release/jlerouge/open-recovr?include_prereleases&sort=semver&label=vers%C3%A3o)](https://github.com/jlerouge/open-recovr/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/jlerouge/open-recovr/total?label=downloads)](https://github.com/jlerouge/open-recovr/releases)
[![Licença MIT](https://img.shields.io/badge/licen%C3%A7a-MIT-blue.svg)](LICENSE)
[![Rust 1.95+](https://img.shields.io/badge/rust-1.95%2B-orange?logo=rust)](https://www.rust-lang.org)
[![Sistemas](https://img.shields.io/badge/sistema-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)](#sistemas-compatíveis)
[![Stand With Ukraine](https://raw.githubusercontent.com/vshymanskyy/StandWithUkraine/main/badges/StandWithUkraine.svg)](https://stand-with-ukraine.pp.ua)

<img src="docs/screenshots/en-4-results.png" alt="Open Recovr mostrando os arquivos excluídos encontrados na unidade C:, com a pré-visualização de uma foto" width="820">

</div>

> [!NOTE]
> Esta página é uma tradução automática e as capturas de tela estão em inglês. Correções de um falante nativo são bem-vindas: veja [CONTRIBUTING.md](CONTRIBUTING.md).

## Por que o Open Recovr?

Perder a pasta das férias ou um TCC já é estressante o bastante. Os programas de recuperação costumam ser caros ou cheios de jargão. Os gratuitos, como o PhotoRec, são excelentes, mas funcionam em um terminal de texto.

O Open Recovr coloca uma interface amigável, de uma só tela, sobre o PhotoRec, um motor livre e comprovado. Ele ainda acrescenta uma análise rápida que traz os arquivos de volta **com o nome e a pasta originais**.

## Como funciona

| 1. Rápida ou profunda | 2. Escolher a unidade | 3. Buscar |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-1-start.png)](docs/screenshots/en-1-start.png) | [![](docs/screenshots/en-2-home.png)](docs/screenshots/en-2-home.png) | [![](docs/screenshots/en-3-scanning.png)](docs/screenshots/en-3-scanning.png) |
| Alguns segundos na Lixeira, ou a unidade inteira. | Disco rígido, SSD, cartão de memória, pendrive, CD… | Os arquivos aparecem à medida que são encontrados, com o tempo restante. |

| 4. Ver e pré-visualizar | 5. Marcar | 6. Recuperar |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-4-results.png)](docs/screenshots/en-4-results.png) | [![](docs/screenshots/en-5-selected.png)](docs/screenshots/en-5-selected.png) | [![](docs/screenshots/en-6-recovered.png)](docs/screenshots/en-6-recovered.png) |
| Filtros por tipo, formato, tamanho ou nome. | Cada arquivo mostra se está intacto. | Copiados para a unidade que você escolher. |

| Configurações | Tema escuro | Modo avançado |
|:---:|:---:|:---:|
| [![](docs/screenshots/en-8-settings.png)](docs/screenshots/en-8-settings.png) | [![](docs/screenshots/en-9-dark.png)](docs/screenshots/en-9-dark.png) | [![](docs/screenshots/en-7-expert.png)](docs/screenshots/en-7-expert.png) |
| Idioma, tema de cores, atualizações, relatório. | Claro, escuro e outros 8 temas. | O registro da busca, para os curiosos. |

O passo a passo completo está no **[guia de uso](docs/user-guide.pt.md)**.

> [!WARNING]
> **Nunca salve os arquivos recuperados na unidade de onde eles vieram.** Você poderia sobrescrever os que ainda esperam para ser recuperados. Um pendrive ou outro disco é perfeito. O Open Recovr avisa se você tentar.

## Recursos

- 🗑️ **Busca imediata** na Lixeira, em segundos e sem permissões especiais.
- 🔎 **Análise rápida (NTFS).** Lê a tabela de arquivos da unidade (MFT), onde um arquivo excluído mantém o nome. Os arquivos voltam **com o nome e a pasta**, em minutos, mesmo depois de esvaziar a Lixeira.
- 🧲 **Análise completa.** O PhotoRec lê a unidade inteira e reconhece **centenas de tipos de arquivo**, mesmo depois de uma formatação ou em cartões FAT/exFAT. Sem o PhotoRec, um mecanismo embutido cuida dos formatos mais comuns.
- 💿 **Mídias danificadas.** As áreas ilegíveis (CD/DVD riscados, cartões gastos) são puladas em vez de travar a busca.
- 🖼️ **Pré-visualização antes de recuperar.** Fotos, vídeos, sons, PDFs, textos e documentos do Word.
- 🏷️ **Nomes e datas que fazem sentido.** Um arquivo que só tinha um número ganha um nome tirado do conteúdo: `photo_2024-07-14_18-22-05.jpg` pela data EXIF, o título de um documento Word ou PDF, o artista e a música de um MP3.
- ✅ **Verificado antes de recuperar.** Cada arquivo encontrado é lido do início ao fim: **Intacto**, **Danificado** (com o motivo: cortado, misturado com outro arquivo, índice ausente…) ou **Vazio**.
- 👯 **Duplicatas ocultas.** O mesmo arquivo encontrado várias vezes aparece uma só vez.
- 📀 **Imagens de disco.** Abre uma cópia `.img` / `.dd` de uma unidade com defeito (feita com o ddrescue, por exemplo), sem tocar no original. Sem direitos de administrador.
- 🗂️ **Ordenação e filtros** por tipo (fotos, vídeos, música, documentos, arquivos compactados), formato, tamanho e nome.
- 💾 **Todo tipo de unidade.** Discos rígidos, SSDs SATA e NVMe, cartões SD, microSD e CompactFlash, pendrives, discos externos, CD/DVD.
- 🕒 **Em segundo plano.** A janela pode se esconder ao lado do relógio durante uma busca longa.
- 🛠️ **Modo avançado.** O registro completo da busca e a saída do PhotoRec.
- 🎨 **Temas de cores.** Claro ou escuro com um clique, e também Nord, Catppuccin, Tokyo Night, Solarized, Monokai e Gruvbox.
- 🔄 **Atualizações seguras.** Quando sai uma nova versão, um clique baixa, verifica a assinatura e instala.
- 🩺 **Relatório de ajuda.** *Preparar um relatório…* salva um zip (sistema, unidades, registro da última busca) para enviar a quem ajuda você, com os nomes pessoais mascarados. O Open Recovr nunca o envia sozinho.
- 🌍 **9 idiomas:** português do Brasil, inglês, francês, espanhol, alemão, italiano, holandês, polonês e ucraniano.
- 🔒 **Somente leitura.** Nada é gravado na unidade analisada. Sem conta, sem telemetria, sem precisar de internet: a única conexão é a procura de atualizações (o número da última versão, pedido ao GitHub), que pode ser desativada nas Configurações.

Um celular conectado por USB não pode ser analisado: ele só mostra os arquivos que ainda tem, e a memória dele é criptografada. O Open Recovr explica onde procurar: a lixeira do app Galeria ou Fotos, o Google Fotos, o iCloud ou o cartão microSD.

## Download

Baixe a versão mais recente em **[Releases](https://github.com/jlerouge/open-recovr/releases/latest)**:

| Sistema | Arquivo | Como iniciar |
|---|---|---|
| **Windows 10 / 11** (64 bits) | `open-recovr-…-windows-x64-setup.exe` ou `…-windows-x64.zip` | Execute o instalador (sem direitos de administrador), ou descompacte e abra `open-recovr.exe`. O PhotoRec está incluído. Os dois se atualizam com um clique. |
| Linux (64 bits) | `open-recovr-…-linux-x64.tar.gz` | Descompacte e execute `./open-recovr`. Para a análise completa: `sudo apt install testdisk`. |
| macOS (Apple Silicon / Intel) | `open-recovr-…-macos-arm64.tar.gz` / `…-macos-x64.tar.gz` | Descompacte, depois clique com o botão direito → *Abrir* (o app não é assinado). Depois, `brew install testdisk`. |

Ler a própria unidade (análise rápida e completa) exige direitos de administrador. O Open Recovr só os pede quando você muda para a busca profunda; a Lixeira é verificada sem eles.

## Sistemas compatíveis

| | Windows | Linux | macOS |
|---|:---:|:---:|:---:|
| Compilado no CI | ✅ | ✅ | ✅ |
| Testado em um computador real | ✅ | ⚠️ ainda não | ⚠️ ainda não |
| Lixeira | ✅ | ✅ lixeira freedesktop | ✅ `.Trash` |
| Tabela de arquivos NTFS (análise rápida) | ✅ | ✅ volumes NTFS | ✅ volumes NTFS |
| Análise completa (PhotoRec) | ✅ incluído | ✅ `apt install testdisk` | ✅ `brew install testdisk` |
| Miniaturas de vídeo / PDF, som | ✅ | ❌ | ❌ |
| Ícone ao lado do relógio | ✅ | ❌ | ❌ |

**O Windows é a plataforma principal e testada.** As versões para Linux e macOS compilam, mas ninguém as testou ainda em um computador real. Relatos e correções são bem-vindos: veja [CONTRIBUTING.md](CONTRIBUTING.md).

## Compilar

Você precisa do [Rust](https://rustup.rs) 1.95 ou mais recente.

```sh
git clone https://github.com/jlerouge/open-recovr
cd open-recovr
cargo build --release
```

No Windows, baixe o PhotoRec uma vez com `powershell -ExecutionPolicy Bypass -File scripts\fetch-photorec.ps1`. As opções de linha de comando estão descritas no [README em inglês](README.md#build-from-source).

## Licença e créditos

O Open Recovr é software livre sob a [licença MIT](LICENSE).

A análise completa é feita pelo **[PhotoRec](https://www.cgsecurity.org/wiki/PhotoRec)** © Christophe Grenier, sob a licença GNU GPL v2 ou posterior. O Open Recovr o executa como um **programa separado, sem modificá-lo**. A versão para Windows inclui o PhotoRec 7.2 com sua licença (`vendor/testdisk-7.2/COPYING.txt`). O código-fonte do PhotoRec está disponível em [cgsecurity.org](https://www.cgsecurity.org/wiki/TestDisk_Download).

O Open Recovr é um projeto independente, sem ligação com nenhum software comercial de recuperação.
