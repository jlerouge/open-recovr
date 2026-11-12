# Guia de uso

[English](user-guide.md) · [Français](user-guide.fr.md) · [Español](user-guide.es.md) · [Deutsch](user-guide.de.md) · [Italiano](user-guide.it.md) · **Português (Brasil)** · [Nederlands](user-guide.nl.md) · [Polski](user-guide.pl.md) · [Українська](user-guide.uk.md)

> [!NOTE]
> Este guia é uma tradução automática e as capturas de tela estão em inglês. Correções de um falante nativo são bem-vindas: veja [CONTRIBUTING.md](../CONTRIBUTING.md).

Este guia acompanha uma recuperação do início ao fim. O aplicativo fala português do Brasil, inglês, francês, espanhol, alemão, italiano, holandês, polonês e ucraniano: o botão de idioma fica no canto superior direito.

- [Antes de começar](#antes-de-começar)
- [1. Busca rápida ou profunda](#1-busca-rápida-ou-profunda)
- [2. Escolher a unidade e o que você procura](#2-escolher-a-unidade-e-o-que-você-procura)
- [3. A busca](#3-a-busca)
- [4. Ver os resultados](#4-ver-os-resultados)
- [5. Marcar o que você quer recuperar](#5-marcar-o-que-você-quer-recuperar)
- [6. Recuperar](#6-recuperar)
- [Imagens de disco](#imagens-de-disco)
- [Configurações, temas e atualizações](#configurações-temas-e-atualizações)
- [Modo avançado](#modo-avançado)
- [Pedir ajuda a alguém](#pedir-ajuda-a-alguém)
- [Perguntas frequentes](#perguntas-frequentes)

## Antes de começar

**Use a unidade o mínimo possível.** Um arquivo excluído continua na unidade até que algo novo seja gravado por cima. Cada download, instalação ou salvamento nessa unidade diminui suas chances.

- Se os arquivos estavam na unidade do sistema (geralmente `C:`), não instale nada nela. Execute o Open Recovr a partir de um pendrive, se puder.
- Se estavam em um cartão de memória ou pendrive, retire-o e não tire mais fotos com ele.
- **Tenha outra unidade pronta** para receber os arquivos recuperados: um pendrive, um disco externo ou um segundo disco interno.

## 1. Busca rápida ou profunda

![A janela «Até onde buscar?»](screenshots/en-1-start.png)

Ao iniciar, o Open Recovr pergunta até onde buscar:

- A **busca rápida** olha só na Lixeira. Leva alguns segundos e não precisa de permissões especiais. Se você esvaziou a Lixeira, ela não vai encontrar nada.
- A **busca profunda** lê a própria unidade. Precisa de direitos de administrador: o Windows pede que você autorize o aplicativo (no Linux e no macOS, sua senha é solicitada).

Você pode mudar depois com o botão **Mudar para a busca profunda** no alto. Marque *Não oferecer mais ao iniciar* se você sempre faz a mesma escolha.

## 2. Escolher a unidade e o que você procura

![A tela inicial](screenshots/en-2-home.png)

**Onde estavam seus arquivos?** Clique na unidade, cartão ou pendrive onde os arquivos estavam. Cada um mostra seu tipo (disco rígido, SSD SATA ou NVMe, cartão SD, pendrive, CD…), seu espaço livre e seu nome.

**Estou buscando:** limita a busca a *Fotos*, *Vídeos*, *Música*, *Documentos* ou *Arquivos compactados*. Menos tipos deixam a busca mais rápida e a lista mais curta. **Refinar** vai além: formatos específicos (só JPG, só DOCX…) e um tamanho mínimo.

Na busca profunda, as unidades NTFS (a maioria dos discos do Windows) oferecem duas análises:

| Análise | O que ela lê | Duração | Mantém os nomes? |
|---|---|---|---|
| **Rápida** | A tabela de arquivos da unidade (MFT) | Alguns minutos | ✅ nome e pasta |
| **Completa** | A tabela de arquivos, depois a unidade inteira com o PhotoRec | De minutos a mais de uma hora | Em parte (veja abaixo) |

Comece pela **Rápida**. Se o seu arquivo não estiver lá, clique em **Não encontrou? Fazer a análise completa**.

Em outros sistemas de arquivos (cartões de memória FAT32 e exFAT, ext4, APFS…), a busca profunda vai direto para a análise completa.

## 3. A busca

![Uma busca profunda em andamento](screenshots/en-3-scanning.png)

Clique em **Buscar os arquivos perdidos**. Os arquivos aparecem na lista **assim que são encontrados**: não é preciso esperar o fim para começar a olhar.

O painel à direita mostra a etapa atual, o progresso, uma estimativa do tempo restante e quantos arquivos foram encontrados até agora.

- **Parar a busca** mantém o que já foi encontrado.
- **Continuar em segundo plano** esconde a janela ao lado do relógio (Windows). Clique no ícone para trazê-la de volta. Uma notificação avisa quando a busca termina.

As áreas ilegíveis, como um CD riscado ou um cartão gasto, são puladas. A busca continua em vez de travar.

## 4. Ver os resultados

![Os resultados, com uma pré-visualização](screenshots/en-4-results.png)

- As **abas** no alto (*Tudo*, *Fotos*, *Documentos*…) mostram quantos arquivos de cada tipo foram encontrados.
- **Formato** e **Tamanho** filtram mais. A caixa de busca filtra por nome.
- Clique no título de uma coluna para **ordenar** por tipo, nome, data ou tamanho.
- Clique em um arquivo para ver a **pré-visualização** à direita, com seus detalhes. Fotos, vídeos, sons, PDFs, textos e documentos do Word podem ser pré-visualizados. **Abrir** o abre no programa de costume, a partir de uma cópia temporária.

Cada arquivo encontrado é lido do início ao fim, e a coluna **Estado** mostra o resultado:

| Estado | Significado |
|---|---|
| **Intacto** | O arquivo foi verificado do início ao fim e está completo. Ele vai voltar como era. |
| **Danificado** | Uma parte está faltando ou errada. O detalhe diz por quê: cortado, misturado com outro arquivo, índice ausente… Talvez ainda abra em parte. |
| **Vazio** | O espaço que ele ocupava só contém zeros. Não há mais nada para recuperar. |

O mesmo arquivo costuma ser encontrado várias vezes (na Lixeira e na unidade, por exemplo). As cópias ficam escondidas atrás de um link *duplicatas ocultas · Mostrar*.

Os arquivos encontrados pelo PhotoRec não têm nome próprio. O Open Recovr dá um nome a eles a partir do conteúdo quando pode: `photo_2024-07-14_18-22-05.jpg` pela data em que a foto foi tirada, o título de um documento Word ou PDF, o artista e a música de um MP3.

## 5. Marcar o que você quer recuperar

![Fotos marcadas para recuperação](screenshots/en-5-selected.png)

Marque os arquivos um a um, ou use a caixa no cabeçalho da coluna para marcar tudo na visualização atual. **Marcar tudo** e **Desmarcar tudo**, embaixo, agem sobre a lista inteira. A barra inferior mostra quantos arquivos estão marcados e o tamanho total.

## 6. Recuperar

![Recuperação concluída](screenshots/en-6-recovered.png)

Clique em **Recuperar a seleção** e escolha uma pasta **em outra unidade**. Se você escolher a unidade analisada, o Open Recovr avisa que você pode sobrescrever outros arquivos recuperáveis.

Os arquivos são copiados para uma nova pasta chamada *Arquivos recuperados* seguida da data, com uma subpasta por tipo (Fotos, Documentos…). Uma pasta enviada para a Lixeira volta inteira. No final, **Abrir a pasta** mostra os arquivos.

Nada é gravado na unidade analisada. Os arquivos são apenas lidos dela.

## Imagens de disco

Uma unidade com defeito (fazendo cliques, muito lenta, cheia de erros de leitura) pode parar de vez durante a busca. É mais seguro copiá-la uma vez, setor por setor, com uma ferramenta como o [ddrescue](https://www.gnu.org/software/ddrescue/), e depois analisar a cópia.

O cartão **Imagem de disco**, ao lado das unidades, abre uma cópia assim (`.img`, `.dd`, `.raw`…). A busca funciona então como em uma unidade, sem direitos de administrador, e o original não é tocado.

## Configurações, temas e atualizações

![A página de configurações](screenshots/en-8-settings.png)

O menu no canto superior direito (☰) reúne tudo o que não é necessário para recuperar arquivos:

- **Configurações**: o idioma, o tema de cores, o modo avançado, se a busca profunda é oferecida ao iniciar, as atualizações e informações sobre o aplicativo.
- **Tema de cores**: claro, escuro, Nord, Catppuccin, Tokyo Night, Solarized, Monokai ou Gruvbox. O botão sol/lua, ao lado do idioma, alterna entre claro e escuro com um clique.
- **Procurar atualizações**, **Preparar um relatório…** e a página do projeto no GitHub.

Quando sai uma nova versão, a barra inferior mostra **→ v…: atualizar**. Um clique baixa a versão, verifica se ela está assinada com a chave do projeto, instala e reinicia o Open Recovr. Se houver uma busca ou recuperação em andamento, a atualização espera ela terminar.

## Modo avançado

![O modo avançado com o registro da busca](screenshots/en-7-expert.png)

O **Modo avançado**, no menu no canto superior direito, mostra o registro da busca embaixo da lista: a unidade e seu sistema de arquivos, cada etapa e as linhas do PhotoRec. **Copiar tudo** copia o registro, útil para relatar um problema. Arraste a borda superior do registro para aumentá-lo.

Cada busca também é gravada em `journal.txt`, junto das configurações (`%APPDATA%\Open Recovr` no Windows). O link **Registro**, embaixo, abre o arquivo.

## Pedir ajuda a alguém

No menu no canto superior direito (☰), **Preparar um relatório…** salva um zip que você pode enviar para quem está ajudando você, por e-mail ou de outra forma. O Open Recovr apenas grava o arquivo: nunca o envia. Dentro há arquivos de texto: o que é o relatório, o sistema e as unidades (modelo, tamanho, sistema de arquivos), as configurações e o registro da última busca. Antes de gravar qualquer coisa, seu nome de usuário, o nome do computador, sua pasta pessoal e os endereços de e-mail são mascarados, assim como os títulos de documentos que o PhotoRec coloca nos nomes dos arquivos que encontra. Abra o zip e leia antes de repassá-lo.

## Perguntas frequentes

**Meu arquivo não está na lista. E agora?**
Verifique se a unidade certa está escolhida e se o filtro de tipo não o esconde (*Tudo*). Depois tente a busca profunda e, em NTFS, a análise completa. Se a unidade foi muito usada desde a exclusão, o arquivo pode ter sido sobrescrito. Aí nenhum programa consegue trazê-lo de volta.

**Os arquivos recuperados se chamam `f1234567.jpg`.**
Esses arquivos foram encontrados pelo PhotoRec, que reconhece um arquivo pelo conteúdo e não pela tabela de arquivos: o nome original é desconhecido. Quando o próprio arquivo não diz nada sobre a data ou o título, o número fica. Ordene por data ou tamanho e use a pré-visualização para encontrar o seu.

**Um arquivo está marcado como Danificado. Vale a pena recuperá-lo?**
Muitas vezes, sim. Uma foto danificada pode mostrar só a metade de cima, um vídeo danificado pode tocar até a parte estragada. Recupere e teste: não custa nada, e a unidade nunca é alterada.

**Ele recupera arquivos do meu celular?**
Não pelo USB. Um celular só compartilha os arquivos que ainda tem, e a memória dele é criptografada. Olhe na lixeira do app Galeria ou Fotos (as fotos excluídas geralmente ficam lá por 30 dias), no Google Fotos ou no iCloud, ou coloque o cartão microSD do celular em um leitor de cartões e analise-o.

**Funciona em um SSD?**
Em parte. A maioria dos SSDs apaga em segundo plano os blocos excluídos (TRIM): arquivos excluídos há algum tempo costumam estar perdidos para sempre. A Lixeira e a tabela de arquivos ainda ajudam em exclusões recentes. Aja rápido.

**Dá para recuperar arquivos de uma unidade formatada?**
Muitas vezes, sim, com a análise completa. Uma formatação rápida deixa o conteúdo no lugar. Uma formatação completa (ou «apagamento seguro») o sobrescreve.

**Por que a busca profunda precisa de direitos de administrador?**
Ela lê a unidade setor por setor, abaixo do sistema de arquivos, e o sistema operacional reserva isso aos administradores. O Open Recovr só lê: nunca grava nessa unidade.

**É seguro? Ele envia meus dados para algum lugar?**
O Open Recovr funciona offline. Sem conta, sem telemetria. A única conexão é a procura de atualizações: ao iniciar, ele pergunta ao GitHub o número da versão mais recente, e não envia mais nada. Desmarque «Procurar atualizações ao iniciar» nas Configurações para desativá-la; o menu ainda pode verificar quando você pedir. Uma atualização só é instalada se estiver assinada com a chave do projeto, que nunca vai para o GitHub: uma versão falsa colocada lá por outra pessoa é recusada. O código é aberto: confira você mesmo.

**Linux / macOS: a busca profunda diz que o PhotoRec não está disponível.**
Instale-o: `sudo apt install testdisk` (Debian, Ubuntu), `sudo dnf install testdisk` (Fedora) ou `brew install testdisk` (macOS). O Open Recovr encontra o `photorec` sozinho. Sem ele, um mecanismo embutido mais simples busca os formatos mais comuns.
