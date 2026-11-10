# Aegis, fork do Templum: estado do trabalho

Atualizado em 2026-11-09. O Aegis é o Templum (MIT) com outro nome e as mudanças deste fork. Este
arquivo reúne as decisões, o que já foi feito e o que falta. A memória do Claude Code fica na
máquina onde a sessão rodou e não vem junto, então **numa sessão nova, comece lendo este arquivo
inteiro**. Ao fechar uma decisão ou terminar uma etapa, atualize-o.

## Objetivos

1. **WSL funcionando normalmente no Aegis para Windows.** Um agente no WSL deve ter tudo o que um
   agente no PowerShell ou no Git Bash tem: status, hooks, ferramentas, medidor de limites, Stop.
2. **App de celular para o Aegis, direto da máquina host para o celular, sem intermediário.** O
   nodeterm faz isso passando pelo relay deles; aqui não pode haver servidor de terceiros no caminho.
3. **Trazer do nodeterm o que valer a pena**, só como ideia: a licença dele (BUSL-1.1) proíbe usar em
   produção uma cópia com as checagens de licença ou de limite removidas, por isso o nodeterm foi
   descartado como base em 2026-11-09.
4. **Trocar o nome do app para Aegis** (decisão 11).

## Repositórios e máquinas

- `origin`: <https://github.com/jpcerchiari/Aegis> (público), desde 2026-11-09.
- `templum-antigo`: <https://github.com/jpcerchiari/Templum> (privado), onde o fork começou, com o
  PR #2 aberto. Fica como cópia antiga; arquivar ou apagar é decisão do usuário.
- `upstream`: <https://github.com/KingDasWinx/Templum> (MIT). Ponto de partida: `development` em
  `9aa691d` (2026-10-29), 3 commits à frente do `main` (`0b97d8a`, v0.2.0).
- **Máquina do trabalho:** tem um clone, mas não dá para compilar lá: o disco está quase cheio (ver
  [Lição do disco do WSL](#lição-do-disco-do-wsl-2026-11-09)). No WSL de lá ficaram instalados: rustup com Rust 1.99.0 e o
  alvo `x86_64-pc-windows-msvc`, `cargo-xwin` 0.23.1, LLVM 21 em `~/.local/opt/llvm-21` e o cache do
  xwin em `~/.cache/cargo-xwin`, uns 3,4 GB ao todo. Tudo isso pode ser removido.
- **PC pessoal:** onde se compila e testa.

## Regras do fork

- **Nenhum código do nodeterm.** Ele é BUSL-1.1, e o Templum é MIT. Do nodeterm vêm só ideias,
  reescritas do zero.
- **Das regras do `CLAUDE.md` do autor, valem as de engenharia:**
  - toda mudança de lógica vem com um teste que falha sem ela;
  - nada de `unsafe`;
  - só `crates/aegis` depende do framework de UI;
  - nada copiado de projeto GPL ou AGPL;
  - toda funcionalidade nova aparece na API, o que já é necessário para o app de celular.
- **Não valem as regras de processo do autor:** PRs para o `development` dele, releases pelo
  `publish.sh`, CHANGELOG gerado.
- **A documentação do fork fica em `docs/fork/`.** A regra de preferir arquivo novo a editar arquivo do
  autor, para os merges doerem menos, caiu com a decisão 11: não há mais merge.
- **Antes de um build grande, medir o espaço livre.** Um build de dev passou de 7,3 GB sem terminar.
- **No GitHub rodam só os nossos workflows:** o `ci.yml` em todo PR e em todo push no `main` (decisão 27) e o
  `release.yml` numa tag `v*` (decisão 25). Os workflows de CI e de release do
  Templum, o `dependabot.yml`, os rulesets e os scripts de release (`install.sh`, `install.ps1`,
  `publish.sh`) foram apagados na troca de nome.
- **Novidades do autor entram à mão** (decisão 11, que substituiu a 8): `git fetch upstream`, ler o que
  mudou (`git log -p <último porte>..upstream/development`) e portar o que interessar. Com os nomes
  trocados, um merge daria conflito em centenas de arquivos. O `upstream/development` estava em
  `9aa691d` quando o Aegis nasceu.

## Decisões

| # | Decisão | Por quê | Situação |
|---|---|---|---|
| 1 | Fork privado, sem contribuir com o autor | Liberdade para mudar; o custo é refazer os merges com o `upstream` à mão | Vale |
| 2 | Hospedagem: começou só local, fora do OneDrive (o OneDrive corrompe a pasta `.git`) | | Substituída em 2026-11-09: repositório privado `jpcerchiari/Templum` no GitHub, com o trabalho continuando no PC pessoal |
| 3 | Build: compilação cruzada a partir do WSL com `cargo-xwin` | Na máquina do trabalho não havia permissão de administrador para instalar o VS Build Tools | Superada pela troca de máquina. No PC pessoal, compilar nativo no Windows (confirmar na primeira sessão) |
| 4 | Toolchain MSVC | Gera o mesmo binário que o autor compila e testa | Vale. No PC pessoal, a licença do Visual Studio Community cobre uso individual |
| 5 | Onde compilar com o C: da máquina do trabalho cheio | | Deixou de valer com a troca de máquina |
| 6 | CI próprio em `.github/workflows/fork.yml`, só com formatação (`cargo fmt`) e licenças (`cargo deny`), no Linux, em todo PR. Build, clippy, testes e executáveis rodam na máquina do usuário, nos três sistemas. Os workflows do autor (`CI` e `Release`) ficam desligados nas configurações do GitHub, sem editar os arquivos dele | Um arquivo novo não dá conflito nos merges. A primeira versão compilava e testava nos três sistemas a cada push num PR e gastou uns 330 dos 2000 minutos do mês numa execução só, sem passar: ver [Achados do primeiro CI](#achados-do-primeiro-ci-2026-11-09) | Substituída pela 27 em 2026-11-09 |
| 7 | Todo build do fork se trata como compilado do fonte (`Install::Manual(Why::Source)`), onde quer que esteja: não procura versão sozinho e nunca se substitui. O *Check now* continua avisando quando sai versão do autor, com o link. A troca é na criação do workspace (`crates/aegis/src/workspace/mod.rs`); o `Install::detect` fica intacto, porque o autostart (`crates/aegis/src/autostart.rs`) também usa, para saber qual executável abrir no login | O atualizador baixa os releases do **autor** (`REPO` em `crates/aegis-update/src/lib.rs:17`). Um build copiado para fora do `target/` (outra pasta no Windows, um `.app` em `/Applications`, um binário fora de `/usr` no Linux) seria trocado pela versão do autor no reinício. O fork não publica releases (decisão 6), então apontar para ele não serve. As novidades do autor chegam pelo merge do `upstream` | Revertida em 2026-11-09 pela decisão 23: o atualizador passou a usar os releases do próprio Aegis. Volta o comportamento original: só o build de dentro de `target/` não se atualiza |
| 8 | As novidades do autor entram por merge do `upstream/development` no `dev_joao`, sem rebase. O passo a passo está em [Regras do fork](#regras-do-fork) | O `dev_joao` já está no GitHub, com PR aberto contra o `main`. O rebase reescreveria commits enviados e exigiria `git push --force` a cada atualização. O fork mexe em poucos arquivos do autor, então os conflitos devem ser pequenos e aparecem uma vez só, no merge | Substituída em 2026-11-09 pela decisão 11: com a troca completa de nome, as novidades do autor entram à mão |
| 9 | WSL em duas etapas. Primeiro PR: as três primeiras linhas da tabela de [Ponto de partida: WSL](#ponto-de-partida-wsl) (o agente recebe servidor, hooks, status line e medidor; o Templum vê o agente; `spawn_agent` aceita). Segundo PR: Stop e contas | As três primeiras dependem umas das outras e já formam o uso do dia a dia. Stop e contas são independentes. Com build e testes só na máquina do usuário (decisão 6), PR menor é mais fácil de conferir | Vale desde 2026-11-09. A implementação fica para o PC pessoal: o código é só de Windows e precisa de um WSL de verdade para testar |
| 10 | No WSL, o Aegis fica sabendo do agente pelos hooks, não lendo os processos da distribuição. O item 1 do primeiro PR faz os hooks atravessarem; o evento do bloco (pelo `AEGIS_TERMINAL`) traz a pasta e a sessão. Ler os processos com `wsl.exe` só entra se faltar algo na prática | Ideia tirada do nodeterm (ver [Como o nodeterm trata o WSL](#como-o-nodeterm-trata-o-wsl)). Não cria mecanismo novo: reaproveita os hooks que o item 1 já entrega. Limites: agente sem hooks não é visto, e um agente que morre sem o hook de fim aparece até o bloco fechar | Vale desde 2026-11-09. Antes de implementar, conferir no código se a bolinha de status e o `spawn_agent` aceitam um agente que só veio pelo hook |
| 11 | O app passa a se chamar **Aegis**, no repositório `jpcerchiari/Aegis` (privado), com troca **completa**: crates (`aegis`, `aegis-terminal`…), variáveis `AEGIS_*`, o comando `aegis`, a pasta de dados, a identidade do app (`jpcerchiari.Aegis`, `io.github.jpcerchiari.aegis`), os arquivos e a documentação. Ficam com o nome do autor: o aviso de copyright no `LICENSE` (com o nosso acrescentado), o `CHANGELOG.md` (histórico do Templum até a 0.2.0) e os links que o atualizador usa (`KingDasWinx/Templum`) | Escolha do usuário. O Templum é MIT: trocar o nome é permitido, desde que o aviso de copyright do autor continue. O custo, aceito: o merge com o autor deixa de ser viável (decisão 8 substituída) | Feita em 2026-11-09, sem compilar (ver Feito) |
| 12 | No WSL, só a distribuição padrão, como o Templum já fazia: um único "WSL" nas listas de shell | O usuário trabalha com uma distribuição só, e o primeiro PR fica menor. Uma entrada por distribuição (como o nodeterm, por `wsl.exe --list --quiet`, deixando de fora as do Docker) pode entrar depois sem desfazer nada | Vale desde 2026-11-09 |
| 13 | O app de celular é um **app web (PWA) servido pelo próprio Aegis**, cliente da API que já existe (eventos, tela e digitação dos terminais, tokens, TLS próprio). O celular abre o endereço do PC no navegador e instala como atalho | Um código só para Android e iOS, sem loja nem build de celular, e o app vem do próprio PC: "sem intermediário" ao pé da letra. Custos aceitos: instalar o certificado do Aegis no celular (*Save certificate…*) e notificação só com o app aberto, porque o Web Push passa pelo Google ou pela Apple. Um app Android pequeno pode vir depois, por cima da mesma API, se o aviso com o app fechado fizer falta | Vale desde 2026-11-09. Falta saber se o celular é Android ou iPhone: no iPhone, o certificado vira um perfil a instalar e a confiar em *Ajustes* |
| 14 | O celular alcança o Aegis pela **rede local e pelo Tailscale**. A API escuta nos dois endereços, sempre com TLS, e o pareamento oferece os dois | Escolha do usuário, para usar fora de casa sem abrir porta no roteador. Aceito conscientemente: o Tailscale é um intermediário na troca de chaves e, quando a conexão direta falha, no tráfego (que vai criptografado pelo WireGuard e pelo TLS do Aegis) | Vale desde 2026-11-09 |
| 15 | O Aegis vira a **própria autoridade certificadora**. Gera uma raiz uma vez (chave privada só do usuário, como a `key.pem` de hoje) e emite com ela o certificado do servidor para `localhost`, `127.0.0.1`, `::1`, os endereços da rede local e o do Tailscale, emitindo outro sozinho quando um endereço muda. O celular instala só a raiz, uma vez. O *Save certificate…* passa a gravar a raiz, e o Aegis to Aegis passa a conferir a impressão digital da raiz | Única opção que atende aos dois caminhos da decisão 14 sem terceiros. **Cuidado:** uma raiz instalada no celular pode assinar qualquer site; se a chave vazar, dá para interceptar o celular. Por isso a raiz leva *Name Constraints* limitando-a a IPs privados (`10/8`, `172.16/12`, `192.168/16`, `100.64/10` do Tailscale, `127/8`, `::1`), e a chave nunca sai do PC. Conferir na implementação se o Android e o iOS respeitam essa restrição numa raiz instalada pelo usuário | Vale desde 2026-11-09 |
| 16 | Pareamento por **dois QR** num diálogo do Aegis. O primeiro abre uma página **HTTP**, só na rede local e só com o diálogo aberto, que entrega a raiz (decisão 15) e mostra a impressão digital, para comparar com a que o Aegis mostra antes de instalar. O segundo abre o app em HTTPS com um **código de uso único**, que o app troca por um token com o alcance escolhido no diálogo (todos os workspaces ou um só; os escopos da API: read, control, full) | Tudo dentro do Aegis, sem e-mail nem mensageiro. O risco do HTTP (alguém na rede trocar a raiz no caminho) some com a comparação da impressão digital, e a página só existe durante o pareamento | Vale desde 2026-11-09 |
| 17 | A primeira versão do app do celular tem **status e terminais**: a lista de agentes com a bolinha de status (`GET /v1/events`) e, ao abrir um, a tela ao vivo (`GET /v1/terminals/{block}/stream`) e a digitação (`terminal_input`). Responder um pedido é digitar no terminal | O terminal resolve tudo, inclusive os pedidos de permissão, e é o recurso principal do nodeterm no celular. A caixa de pedidos com botões (`POST /v1/requests/{id}/answer`) fica para uma segunda versão. Digitar exige token **full**: no diálogo de pareamento (decisão 16), essa escolha tem que ser explícita | Vale desde 2026-11-09 |
| 18 | **Ordem do trabalho:** 1) WSL, primeiro PR (decisões 9, 10 e 12); 2) celular, primeira versão (13 a 17); 3) não deixar o PC dormir enquanto um agente trabalha; 4) paleta de comandos; 5) depois, avaliar sessões que sobrevivem ao fechar o app. **Fora:** ditado com Whisper, projetos por SSH, blocos de página web e de vídeo, GitHub Issues no kanban | Os dois objetivos que motivaram o fork vêm primeiro. O item 3 protege o celular: com o PC dormindo, ele perde o acesso, inclusive pelo Tailscale. Itens 3 e 4 são pequenos e não existem no Aegis (conferido no código em 2026-11-09). O item 5 é valioso, mas no Windows exige um processo separado segurando os terminais | Vale desde 2026-11-09 |
| 19 | O celular principal é **Android**, e o app tem que funcionar também no **iPhone**. A página do pareamento (decisão 16) entrega a raiz nos formatos que os dois aceitam: `.crt` (DER, `application/x-x509-ca-cert`) para o Android e `.mobileconfig` para o iPhone, cada um com o passo a passo na tela. No Android 11 em diante, o navegador só baixa o arquivo, e a instalação é em *Configurações ▸ Segurança ▸ Criptografia e credenciais ▸ Instalar certificado ▸ Certificado CA*. No iPhone, instalar o perfil e depois ligar a raiz em *Ajustes ▸ Geral ▸ Sobre ▸ Ajustes de Confiança de Certificados* | Resposta do usuário. O app web (decisão 13) já atende os dois; o que muda é o pareamento e onde testar: no Android, no aparelho; no iPhone, no Simulador do Xcode no Mac do usuário, até haver um aparelho | Vale desde 2026-11-09 |
| 20 | O Aegis **não tem tarefa no gerenciador de tarefas do trabalho** | Projeto pessoal | Vale desde 2026-11-09 |
| 21 | O Aegis é **independente no GitHub**: o histórico começa num commit único, sem os commits do Templum, e o `README.md` não cita o Templum (sem atribuição, vídeos de demonstração, releases, AUR nem CHANGELOG; a instalação é só pelo código-fonte). O histórico antigo continua no `jpcerchiari/Templum` e nos remotes locais `upstream` e `templum-antigo`, que servem para portar novidades à mão e nunca são enviados ao Aegis | Pedido do usuário. O que a licença MIT exige continua: o aviso de copyright do autor no `LICENSE` | Vale desde 2026-11-09 |
| 22 | Fora do README, **sai o que não é código**: o `CHANGELOG.md` começa no Aegis, o link do branch `spike/gpui` sai do `docs/adr/0002-ui-framework.md`, e saem de `docs/minors.md` as 5 entradas sobre arquivos apagados (instaladores, release, assinatura de downloads). **Ficam:** o `LICENSE` (exigência da MIT), este documento (as decisões não fazem sentido sem a origem) e o autor no `authors` do `Cargo.toml`, por cortesia. O atualizador **fica**, apontado para os releases do Aegis (decisão 23) | Pedido do usuário | Vale desde 2026-11-09 |
| 23 | O atualizador busca as versões novas nos **releases do próprio `jpcerchiari/Aegis`**, que passa a ser **público** (a troca de visibilidade é do usuário). Sem token: o link comum de download funciona em repositório público. `REPO` em `crates/aegis-update/src/lib.rs`, a fixture e os testes apontam para `jpcerchiari/Aegis`; a decisão 7 foi revertida (`Install::detect()` de volta) | O usuário quer um atualizador próprio, independente do Templum. Reaproveita o atualizador inteiro em vez de removê-lo. Falta o processo de release: sem release publicado, o *Check now* não acha nada | Vale desde 2026-11-09. Feito sem compilar |
| 24 | Antes de o repositório ficar público, este documento deixou de citar o empregador do usuário, o caminho e o usuário na máquina do trabalho e o gerenciador de tarefas de lá. O repositório é recriado do zero, para nenhum commit antigo ficar acessível pelo hash | O repositório público é lido por qualquer pessoa. Depois de um force push, o GitHub ainda serve por um tempo um commit antigo a quem tiver o hash, então recriar é o único jeito de garantir | Vale desde 2026-11-09 |
| 25 | Os **releases são compilados no GitHub Actions**, pelo nosso `.github/workflows/release.yml`, quando uma tag `vX.Y.Z` é enviada: cinco alvos (macOS Apple Silicon e Intel, Linux x86_64 e ARM, Windows x86_64), com os arquivos que o atualizador baixa, um `.sha256` ao lado de cada um e as notas tiradas da seção da versão no `CHANGELOG.md`. O disparo manual é um ensaio, sem release. O CI de PR é outra decisão: a 6, depois a 27 | Repositório público: o Actions é de graça e usa máquinas de 4 núcleos, então o motivo da decisão 6 não vale para os releases. Sai igual toda vez e não exige estar nos três computadores | Vale desde 2026-11-09. Escrito a partir do workflow do autor, sem rodar ainda |
| 26 | O Aegis começa a própria numeração: o primeiro release é o **`v0.1.0`**. `[workspace.package] version` em `0.1.0`, o `Cargo.lock` atualizado só nas 9 entradas do Aegis, e o `CHANGELOG.md` com a seção `## 0.1.0` | Coerente com o Aegis independente. Nada dependia da numeração do Templum: a pasta de dados e a identidade do app já são outras, e os testes usam versões escritas à mão | Vale desde 2026-11-09 |
| 27 | O CI de PR volta a **compilar e testar nos três sistemas**: `.github/workflows/ci.yml` (o antigo `fork.yml`, renomeado) roda em todo PR e em todo push no `main`. Formatação e licenças no Linux; build, clippy com `-D warnings` e testes no Windows, no macOS e no Linux. Os testes rodam pelo `cargo nextest` com `--retries 2`, que repete o que falhou e marca como FLAKY sem reprovar; os doctests, que o nextest não roda, pelo `cargo test --doc`. O cache só é salvo no `main`, com `cache-on-failure` | O motivo da decisão 6 (minutos do plano, máquinas de 2 núcleos) deixou de valer com o repositório público. O `--retries 2` responde aos testes do autor que falham ao acaso (ver [Achados do primeiro CI](#achados-do-primeiro-ci-2026-11-09)). PR só usa o cache dele mesmo e o do `main`, e cache de PR gastaria a cota de 10 GB do repositório | Vale desde 2026-11-09. Escrito a partir do CI do autor, sem rodar ainda |
| 28 | **Identidade própria:** o ícone é o escudo do item Aegis de *Risk of Rain 2*, desenhado do zero em SVG (`crates/aegis/icons/icon.svg`, e `icon-small.svg` para 16 a 48 px), reconhecível pelos traços do item: escudo facetado prateado, capuz e aro em V dourados, o anel na haste e, em volta, a barreira como uma casca de luz dourada. Estilo de ícone de app: sem contorno, volume por luz, sombra e espessura do metal, e um fundo roxo iluminado de cima. Sai o `icon-dark.svg`: o fundo escuro serve nos dois temas. O README fala do próprio Aegis (o uso pensado, o roadmap da decisão 18) e tem no cabeçalho a frase do item, *"Healing past full grants you a temporary barrier."*, com uma nota de que o projeto não tem vínculo com o jogo | Pedido do usuário: o ícone tinha que ser reconhecível como o item, não um logo simplificado. Desenho próprio, não decalcado: a arte do item é da Hopoo Games, e o repositório público distribui releases | Vale desde 2026-11-09 |

## Feito

- **Comparação entre nodeterm e Templum.** O nodeterm é mais maduro, mas é BUSL-1.1, tem plano pago
  a caminho (limite de 5 projetos abertos, US$ 10/mês no Pro) e passa pelo relay deles no celular. O
  Templum é MIT e nativo, mas perde as integrações quando o agente roda no WSL. Para uso pessoal com
  liberdade de mexer no código, o Templum foi escolhido como base.
- **Mapeamento do que falta no WSL.** Ver [Ponto de partida: WSL](#ponto-de-partida-wsl).
- **Mapeamento da API que o celular vai usar.** Ver [Ponto de partida: celular](#ponto-de-partida-celular).
- **Tentativa de compilação cruzada.** Não terminou. Os achados ficam em
  [Achados da compilação cruzada](#achados-da-compilação-cruzada), caso ela volte a ser útil.
- **Troca de nome para Aegis** (decisão 11), feita por script em 2026-11-09: 403 arquivos de texto,
  9 pastas de crate e 13 arquivos renomeados; o `Cargo.lock` foi regerado sem mudar nenhuma versão
  (`cargo update --workspace --offline`, conferido pacote a pacote e com `--locked`), o `cargo fmt`
  passou e todos os `include_str!` apontam para arquivos que existem. **Não foi compilado**: conferir
  com a linha de base no PC pessoal. Testes que dependem do tamanho ou da ordem alfabética do nome
  ("Templum" tinha 7 letras, "Aegis" tem 5) podem falhar. O primeiro CI (2026-11-09, só o Windows rodou) achou
  um: `tap::tests::a_second_integrations_marks_are_the_same_prompts_and_command` procurava a marca pelo começo
  de `templum:` (`\x1b]8;;te`); agora monta o trecho a partir de `URI`.
- **Atualizador apontado para os releases do Aegis** (decisão 23), sem compilar.
- **Workflows do Actions, commit `fd6564a`:** no [CI](https://github.com/jpcerchiari/Aegis/actions/runs/42680529502),
  formatação, licenças, Windows e macOS passaram. O Linux compilou e passou no Clippy. Dos 3.442
  testes Linux, `inspect_finds_an_agent_the_shell_started` falhou nas três tentativas e
  `inspect_finds_an_agent_and_its_folder` passou na terceira. Os doctests de Linux ficaram pendentes.
  O [ensaio de release](https://github.com/jpcerchiari/Aegis/actions/runs/42682578389) empacotou os cinco
  alvos. A etapa de publicação ficou de fora porque o disparo foi manual.
- **Linha de base local no Mac Apple Silicon:** Rust 1.99.0. Formatação, build do workspace
  e Clippy com `-D warnings` passaram. O binário abriu, renderizou e fechou normalmente no
  ensaio gráfico `--bench-zoom=empty`. A suíte completa teve três falhas. O teste de
  permissões de `aegis-git` pegava o `ls` personalizado do shell de login. Após usar
  `command ls` nesse teste, os 210 testes da crate passaram. Duas falhas se repetiram
  isoladamente: `stop_agent_ends_what_an_agent_that_quit_left_running` e
  `strays_are_the_program_what_it_left_in_a_session_of_its_own_and_never_the_shell`.
  Uma sondagem local confirmou que o `sysinfo` vê os argumentos de `/bin/sh` e `/bin/sleep`
  mas recebe o ambiente vazio. Uma cópia de `sleep` no diretório temporário expõe a variável
  `AEGIS_TERMINAL`. A busca por processos órfãos depende dessa variável.

## Ponto de partida: WSL

O próprio autor já documentou as lacunas: a seção *What works in WSL* em `docs/specs/shells.md` e
várias entradas em `docs/minors.md`. Hoje, um agente marcado com o shell WSL roda em `wsl.exe`, sempre
na distribuição padrão e no shell padrão dela.

| Lacuna no ponto de partida | Onde estava no código | Correção sugerida pelo autor |
|---|---|---|
| Servidor do Templum, hooks, status line e medidor de uso: o agente no WSL não recebe nada disso | `Workspace::gives` (`crates/aegis/src/workspace/usage.rs:245`) devolve `None` quando o shell é `wsl` | Listar `AEGIS_BUS`, `AEGIS_TERMINAL` e os `AEGIS_*_ARGS` em `WSLENV` (com `/p` nos caminhos); passar os arquivos do Claude Code e do Codex como caminhos `/mnt/c/…`; rodar a ponte MCP como `aegis.exe` pelo interop, que alcança o loopback do Windows |
| O Templum não vê o agente: sem bolinha de status, sem aviso ao fechar o bloco, e o agente some na reinicialização | `aegis_terminal::inspect` só lê processos do Windows; o snapshot fica em `Space::see` (`crates/aegis/src/workspace/mod.rs`) | Ler os processos da distribuição (`wsl.exe -e ps -o pid,args`, ou `\\wsl.localhost\<distro>\proc`) e casar com o bloco pela sessão do shell |
| `spawn_agent` e o Templum Agent recusam agente no WSL | `Workspace::spawn_problem` | Resolvido junto com a linha acima: o Templum precisa ver o agente iniciar para digitar o prompt |
| O Stop não alcança programas dentro do WSL | `tree` (`crates/aegis-terminal/src/terminal.rs`) | `wsl.exe -e pkill -s <sessão>`, ou `wsl.exe --terminate` numa distribuição usada só por aquele workspace |
| Contas não funcionam no WSL: a pasta da conta vai por variável, que não atravessa | `crates/aegis/src/settings/accounts.rs` | Também pelo `WSLENV` |
| Pendências menores: a nota de WSL no diálogo de agentes repete `Workspace::in_wsl`; falta teste para a linha "in WSL" | `crates/aegis/src/settings/agents.rs:267-274` | Um helper único em `Workspace` |

Arquivos que mais mencionam WSL: `aegis-terminal/src/integration.rs`, `workspace/blocks.rs`,
`settings/agents.rs`, `workspace/bus.rs`, `aegis-terminal/src/shells.rs` e `workspace/usage.rs`.

### O que o primeiro PR mexe (decisão 9)

**Progresso no PC pessoal (2026-11-09).** Ubuntu é a distribuição padrão no WSL 2. O Claude Code
2.1.311 foi instalado no Ubuntu. O build nativo passou com Rust 1.99.0, MSVC e Windows SDK. O teste em
ConPTY abriu a distribuição em uma pasta com espaços, preservou uma entrada própria de `WSLENV`
e confirmou que um executável Windows chamado do Linux recebe o ID correto do bloco.
No Aegis compilado, dois blocos WSL na mesma pasta receberam hooks de teste separados e
`aegis.exe whoami` alcançou o barramento do Windows. Um bloco PowerShell serviu de controle.
O snapshot seguinte manteve os dois IDs de sessão, e o `SessionEnd` limpou só o bloco que o
enviou. O relay da linha de status publicou o
uso do bloco na API. Um agente simulado recebeu um único prompt após o hook de início. Ao
reabrir o perfil de teste, o Aegis emitiu `claude --resume` com o ID salvo antes do fechamento.

Formatação e Clippy passaram. A suíte do aplicativo teve 2.073 testes aprovados, um ignorado e
uma falha em `shortcuts_open_and_close_terminals` na asserção da animação da câmera. O teste
passou depois, rodado isoladamente tanto no `HEAD` quanto no código com a integração WSL.
Nas demais crates, `a_noise_flood_never_rotates_the_calls` falhou em lote e passou isoladamente.

O código usa `WSLENV` com `/p` para os caminhos lidos pelo Linux. O Bash da distribuição reutiliza
a integração de terminal para os comandos digitados e chama `wslpath` para informar a pasta em
formato Windows. A identidade de Claude Code e Codex vem dos hooks e entra no snapshot do canvas.
O relay do Claude Code lê o uso por bloco sem exigir um PID Windows. A consulta a `/usage`
retornou os limites da conta autenticada usando o Claude instalado no Ubuntu. O teste interativo
confirmou que `SessionStart` vem depois da confiança na pasta e antes do primeiro prompt. O
`UserPromptSubmit` registrou um envio, e o limite da conta impediu a resposta do modelo. O
`spawn_agent` usa esse hook e a integração do Bash para confirmar prontidão no Claude Code.
Para Codex, a prontidão do primeiro prompt continua pendente. Stop e contas fazem parte da
segunda etapa.

O levantamento abaixo partiu do commit `fd6564a` e registra a sequência e os critérios de teste.

1. **Transportar a configuração.** Em `Terminal::spawn`, somar ao `WSLENV` existente apenas as
   variáveis usadas pelo bloco WSL, incluindo `AEGIS_BUS` e `AEGIS_TERMINAL`. Preservar o `PATH`
   do Linux e acrescentar apenas a pasta da ponte se ela precisar estar no caminho. Confirmar o
   transporte com `aegis.exe whoami` em um bloco, sem imprimir `AEGIS_BUS`, que contém o segredo
   local no Windows. As duas variáveis precisam atravessar também na volta, do WSL para
   `aegis.exe`. O executável acessa o loopback do próprio Windows via interop.
   [Referência da Microsoft sobre WSLENV](https://learn.microsoft.com/en-us/windows/wsl/interop#wslenv-flags).
2. **Gerar as configurações para o shell.** Em `workspace/usage.rs`, `Workspace::gives` alimenta
   `typed_in`, `agent_env` e `server_vars`. Gravar os arquivos no host e usar `wslpath` da
   distribuição padrão, fora da thread da interface, para os caminhos que o agente Linux lê.
   Usar aspas de shell Unix. O relay `aegis.exe` roda no Windows, portanto o caminho do diretório
   onde ele grava deve continuar sendo do Windows. Conferir também quem usa o nome simples
   `aegis` (por exemplo, os hooks do Codex) e o lançamento digitado à mão. Hoje
   `integration::inject` não instala as funções de shell no WSL, então passar os `AEGIS_*_ARGS`
   sozinho não faz o comando digitado consumi-los.
3. **Manter a identidade do agente e a sessão.** Pela decisão 10, começar pelos hooks. Hoje
   `AgentEvent::Started` sinaliza que o agente está ocioso, mas não preenche `pane.seen`.
   A mensagem que chega ao workspace já perdeu o tipo do agente, a pasta e o ID da sessão
   do payload original. `Space::see` apaga o estado quando a inspeção do host não vê agente.
   Levar esses dados junto com o evento no WSL, manter o estado entre snapshots e limpar no
   fim da sessão. `Layout::snapshot` faz sua própria inspeção e `session_ids` olha os arquivos
   do host. Conferir o caminho inteiro
   até o salvamento e a retomada da mesma sessão. O relay de uso já é por terminal, mas
   `Workspace::proc_of` exige PID e horário de início. Separar os dados vindos do relay dos
   que dependem de um processo Windows real. O medidor de limites do Claude Code também
   consulta `/usage` em `claude_poll_jobs`, que hoje procura o executável no Windows. Conferir
   o medidor com Claude Code instalado somente na distribuição.
4. **Liberar `spawn_agent` depois de confirmar prontidão.** `look_ready` exige um PID de agente,
   tela estável e nenhum pedido de permissão reportado. O código registra que o `SessionStart`
   do Codex pode depender do primeiro prompt. Exercitar esse caso no WSL antes de tirar a
   recusa de `spawn_problem`.
   Se o hook chegar tarde demais, investigar um sinal de prontidão da distribuição antes de
   liberar o envio. O prompt nunca pode cair no shell ou responder à confirmação de confiança
   do agente.

**Verificação no Windows.** Usar a distribuição padrão e testar em uma pasta com espaços.
Conferir uma variável própria que já constava no `WSLENV`, um bloco PowerShell como controle e
dois blocos WSL para provar que o evento chega ao bloco certo. Usar a entrada nativa Claude
Code com o shell WSL para testar as flags de `typed_in`. Abrir pelo canvas e digitar o comando
em outro bloco. Conferir ferramentas MCP, status no canvas e em `GET /v1/events` após um
snapshot, medidor de limites e retomada ao reabrir o Aegis. Ao sair do agente normalmente,
conferir que o bloco volta a aparecer como terminal.
Em `spawn_agent`, verificar que o prompt chega uma vez após a confirmação de confiança.
Repetir com Codex antes de habilitá-lo por essa rota. Testar também uma falha de tradução
ou interop desabilitado, sem travar a janela.
Os testes automatizados existentes em `workspace/tests/agents.rs`, `bus.rs` e `spawn.rs` são
os pontos de partida para cobrir os caminhos nativo e WSL.

## Ponto de partida: celular

### Cliente móvel em teste

O Aegis já serve o cliente em `/mobile/` quando a API está em HTTPS. Em **Settings ▸ API**,
ative a API, use um certificado que o navegador confie para o endereço escolhido e crie um
token. Abra `https://<endereço>:7411/mobile/` (ou a porta configurada) e cole o token.
`Read` acompanha agentes e terminais. `Full` também permite colar texto, digitar caracteres
e enviar teclas. O token fica na memória da página e precisa ser informado de novo após
recarregar. A página e os ícones são públicos nesse endereço. A API continua exigindo o token
e aplicando seu escopo e a lista de workspaces.

O cliente usa a mesma origem para os eventos e a tela do terminal, sem bibliotecas externas.
O ensaio em Chrome com viewport móvel verificou os critérios de instalação do manifesto,
os escopos, mudanças no status do agente e nas linhas do terminal, envio de texto e teclas
e revogação do token. A instalação em Android e iPhone ainda precisa ser testada.

**Próximo passo do celular.** Implementar a autoridade certificadora e a renovação da folha
para LAN e Tailscale (decisões 14 e 15). Depois, ligar o pareamento por dois QR e a troca de
código de uso único à página (decisões 16 e 19). Até lá, a configuração de TLS e do token é
manual.

### API existente

Metade do caminho já existe no Templum, sem servidor intermediário (`docs/api.md` e `docs/specs/api.md`):

- **A API** fica desligada por padrão. Ligada, escuta em `127.0.0.1:7411`, e fora do loopback só
  aceita TLS, com certificado próprio verificado pela impressão digital. Já existe o botão *Save
  certificate…*, que grava um `.pem` justamente para instalar num celular.
- **Tokens** têm escopos e podem ser limitados a um workspace. O Templum guarda só a impressão
  digital do token.
- **Estado ao vivo:** `GET /v1/events` (Server-Sent Events) começa com um snapshot e segue com as
  mudanças.
- **Terminais:** `GET /v1/terminals/{block}/stream` envia a tela do terminal, e `terminal_input`
  digita texto ou teclas nele.
- **Aegis to Aegis** (`docs/specs/aegis-to-aegis.md`, o "Templum to Templum" do autor): espelha um workspace em outra máquina
  a partir de um *código de conexão*, com dois modos: `pinned`, conexão direta que fixa o certificado,
  e `public`, por túnel. Esse código de conexão serve de base para o pareamento por QR code.

O app de celular seria mais um cliente dessa API, falando direto com o PC.

**Achados para o app web (decisões 13 e 14):**

- **O certificado só vale para `localhost`, `127.0.0.1` e `::1`** (`make` em
  `crates/aegis-api/src/tls.rs`). O navegador do celular recusa o endereço da rede local e o do
  Tailscale mesmo com o certificado instalado, porque o nome não confere; e um PWA (service worker,
  instalação) só funciona com certificado válido. O endereço da rede local ainda muda com o DHCP.
  Para o Aegis to Aegis isso não pesa, porque ele confere pela impressão digital.
- **Um PWA fica preso a um endereço (origem).** Instalado pelo endereço do Tailscale, ele serve em
  casa e fora: em casa o Tailscale usa o caminho direto pela rede local. Pelo endereço da rede local,
  seria uma segunda instalação, com o próprio token, para quando o Tailscale do celular estiver
  desligado.

## Ideias do nodeterm

Conferido só no `docs/guide.md` do Templum. Onde a tabela diz "não achei", confirmar no código antes de
decidir.

| Ideia | No Templum hoje | Avaliação inicial |
|---|---|---|
| Sessões que sobrevivem ao fechar o app (no nodeterm, via tmux) | Não: o Templum reabre os terminais na última pasta e retoma a última conversa do agente, mas o processo morre quando o app fecha | Forte candidato. No Windows não existe tmux; seria preciso um processo separado que segure os terminais |
| Impedir que o PC durma enquanto um agente trabalha | Não achei | Candidato pequeno |
| Ditado local com Whisper | Não achei | Candidato médio |
| Aprovar o pedido de permissão do Claude Code no próprio bloco | Existe `ask_user` e a caixa de pedidos (`docs/specs/ask-inbox.md`, `ask-user.md`); não confirmei se cobre o prompt de permissão do Claude Code | Verificar |
| Paleta de comandos | Não achei | Candidato pequeno |
| Projetos remotos por SSH | Não; existe só o Templum to Templum | Avaliar |
| App web servido pelo próprio host (Server Edition do nodeterm) | Não | Relevante como forma de entregar o app de celular |
| Blocos de página web e de vídeo | Não achei | Baixa prioridade |
| GitHub Issues no kanban | Não | Baixo valor para o uso pessoal |
| Cards de subagentes, kanban ligado a agentes, Git com commit gerado por IA, editor, diff, notas, contas, worktrees, fork de conversa | Já existem | Nada a trazer |
| Notch do macOS | | Não se aplica |

### Como o nodeterm trata o WSL

Lido em 2026-11-09 no `docs/windows-session-host.md` dele e no código encontrado pela busca do GitHub.
Só a ideia; nada de código (BUSL-1.1).

- **O WSL é um tipo de terminal, um por distribuição.** A lista sai de `wsl.exe --list --quiet`, cuja
  saída vem em UTF-16. A pasta do projeto é traduzida pelo `wslpath` da própria distribuição, em vez
  de montar `/mnt/c/…` à mão (a raiz de montagem muda com o `/etc/wsl.conf`). O terminal abre com
  `wsl.exe -d <distro> --cd <pasta>` e confere a pasta antes, para uma pasta apagada não abrir em `/`.
- **O estado do agente vem dos hooks, não dos processos.** Um script nas configurações do agente
  manda cada evento para o servidor local do app, e só quando o terminal tem a variável que
  identifica o bloco. O Templum já faz parecido: `aegis hook <agent>` manda os eventos para o bus,
  e eles acendem a bolinha de status (`docs/specs/connections.md`, Hooks and agent state).
- **Os hooks não atravessam para o WSL.** A busca não achou `WSLENV` no código: a variável do bloco
  não chega ao shell do WSL, e o script não manda nada. Além disso, um `curl` para `127.0.0.1`
  dentro do WSL 2, na rede padrão (NAT), cai no loopback do Linux, não no do Windows. Pelo que
  achei, o nodeterm no Windows (beta) tem a mesma lacuna que o Templum.
- **O que vale trazer:** o `wslpath` da distribuição para traduzir os caminhos (item 1 do primeiro
  PR), o `--cd` com a pasta conferida, a lista por `--list --quiet` (para a pergunta de qual
  distribuição) e a ideia de saber do agente pelos hooks (Pergunta 11).

## Próximos passos

1. **Preparar o PC pessoal** (Windows):
   - `git clone https://github.com/jpcerchiari/Aegis`, depois
     `git remote add upstream https://github.com/KingDasWinx/Templum`. Partir do `main` atualizado.
   - instalar o rustup (<https://rustup.rs>); a versão do Rust fixada no `rust-toolchain.toml` se
     instala sozinha;
   - instalar o Visual Studio Build Tools com o workload *Desktop development with C++*, incluindo o
     Windows SDK: o build de release usa o `fxc.exe` dele para compilar os shaders;
   - **deixar folga no disco:** o build de dev passou de 7,3 GB sem terminar; reserve uns 30 GB (é
     uma estimativa);
   - **para o WSL (decisão 9):** conferir a distribuição padrão com `wsl.exe --list --verbose` e
     o Claude Code dentro dela com `wsl.exe --exec sh -lc "command -v claude"`.
2. **Linha de base antes de mudar a integração no Windows:** abrir com `cargo run -p aegis -- --dev`
   e executar `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`
   e `cargo test --workspace --locked`. O CI já verifica as três plataformas (decisão 27).
   Para isolar a falha vista no Linux em `fd6564a`, rodar
   `cargo test -p aegis-terminal --locked --test spawn inspect_finds_an_agent_the_shell_started -- --exact --nocapture`.
   Se falhar, capturar a saída do PTY e os PIDs do shell e do grupo de primeiro plano antes de
   atribuir a falha ao timeout ou alterar a inspeção.
3. **Publicar um release** (decisão 25):
   1. em `Cargo.toml`, `[workspace.package]`, mudar `version`; depois `cargo update --workspace`, que
      só atualiza as entradas do próprio Aegis no `Cargo.lock`;
   2. no `CHANGELOG.md`, trocar `## Unreleased` por `## X.Y.Z`;
   3. commit, `git tag vX.Y.Z` e `git push origin main vX.Y.Z`. O Actions compila e publica.
4. **Implementar na ordem da decisão 18**, começando pelo WSL. As decisões de arquitetura estão
   fechadas (1 a 20). Dúvidas que aparecerem no caminho seguem o grill-me: uma pergunta por vez,
   registrada aqui.

## Achados da compilação cruzada

Só importam se a compilação cruzada voltar a ser usada.

- **Shaders do DirectX:** `gpui-fast-windows` 0.1.8 e `gpui-pre-windows` 0.3.9 compilam o HLSL com o
  `fxc.exe` no `build.rs`, mas só quando a máquina que compila é Windows e o build é de release. Num
  build cruzado de release, o arquivo gerado não existe e o build nem compila. No build de dev, o
  shader é compilado na abertura do app, via `D3DCompileFromFile`, lendo
  `env!("CARGO_MANIFEST_DIR")/src/shaders.hlsl`, que num build cruzado é um caminho do Linux. Para
  contornar, seria preciso alterar esses dois crates para compilar a partir do fonte embutido, com
  `D3DCompile`. O `d3dcompiler_47.dll` vem com todo Windows. Existe também a variável
  `GPUI_FXC_PATH`.
- O `ring` (C) precisa do `clang-cl`, o `embed-resource` precisa do `llvm-rc`, e o `cargo-xwin` baixa
  a biblioteca C e o SDK da Microsoft, uns 1,1 GB de cache.

## Achados do primeiro CI (2026-11-09)

Execução 42627312052, no PR #2, com a primeira versão do `fork.yml` (hoje `ci.yml`), que compilava e testava nos três
sistemas. O fork ainda não mudava código, então nada do que falhou veio dele. Depois dela, o CI ficou
só com formatação e licenças (decisão 6), e voltou a compilar e testar quando o repositório ficou público
(decisão 27).

- **Os testes do autor falham ao acaso.** Olhei 3 execuções recentes do CI dele que falharam. Em cada
  sistema que falhou, foi um teste só, e sempre um diferente. São testes que esperam um processo ou
  uma tela por 10 a 30 segundos. No nosso, falharam 3 testes de PowerShell no macOS (o .NET não criou
  a pasta `.dotnet/lockfiles` do temporário) e 2 do `tests/spawn.rs` no Linux. Os mesmos testes
  passam no CI do autor, com a mesma imagem de macOS. Vale também na máquina local: um teste que
  falha e passa quando roda de novo é, provavelmente, um desses.
- **Runner de repositório privado é mais lento.** Linux e Windows têm 2 núcleos, contra 4 nos
  repositórios públicos. Sem cache, o Linux levou 32 min de build, 10 de clippy e 18 de testes, e
  bateu o teto de 60 min; o Windows também. No CI do autor, com o cache completo, o build leva 42 s.
- **Cache:** o `rust-cache` só salva quando o job passa (a menos que se use `cache-on-failure`), e o
  GitHub só deixa um PR usar o cache dele mesmo e o do `main`. Sem testes no `main`, o primeiro push
  de cada PR novo começaria do zero.
- **Custo dessa execução:** uns 330 minutos do plano, se valem os multiplicadores de sempre (Windows
  conta 2 vezes, macOS 10): Linux 60, Windows 60 × 2 e macOS 15 × 10.

## Lição do disco do WSL (2026-11-09)

Um build de dev cruzado, rodando dentro do WSL, encheu o C: e travou o Windows. O
disco do WSL é um arquivo `ext4.vhdx` dentro do C:, e o `df -h /` mostra o teto de 1 TB desse disco
virtual, não o espaço livre de verdade. O espaço real se mede com `df -h /mnt/c`. Nenhum repositório
foi danificado: o `git fsck` passou em todos. Como o vhdx não é esparso, o espaço só volta ao C:
compactando o disco virtual com `diskpart`, o que exige administrador.
