# Plume et WordCraft : rester synchronisé

> **Date :** 2026-10-20 · **Statut :** procédure en vigueur

Plume (`TheLazyGeekGuy/plume`) est un fork de WordCraft (`storytold/wordcraft`). Il suit l'original
en le **fusionnant** régulièrement, sans jamais rebaser : chaque conflit n'est résolu qu'une fois et
l'historique commun reste intact, ce qui garde aussi possible le renvoi de correctifs vers l'original.

## Les branches

| Branche | Rôle |
|---|---|
| `main` | Plume, ce qui est publié. Ne reçoit que des pull requests. |
| branches de travail | Une par chantier, fusionnées dans `main` par pull request (commit de fusion, pas de squash). |
| `sync/upstream-AAAAMMJJ` | Créées par le workflow de synchronisation, une par fusion de l'original. |

Remotes d'un clone : `origin` → `TheLazyGeekGuy/plume`, `upstream` → `storytold/wordcraft`
(`git remote add upstream https://github.com/storytold/wordcraft`).

**Ne jamais** utiliser « Sync fork › Discard commits » sur GitHub : il remplace `main` par l'original
et efface Plume. Le bouton « Update branch » n'est sûr que lorsqu'il n'y a pas de conflit.

## Chaque semaine (automatique)

`.github/workflows/upstream-sync.yml` tourne le lundi (et à la demande, onglet Actions › Upstream
sync › Run workflow) :

- fusion sans conflit : une pull request « Sync upstream WordCraft (N commits) » s'ouvre vers `main` ;
  lancer `cargo xtask ci` sur sa branche, puis la fusionner par commit de fusion ;
- fusion en conflit : une issue liste les fichiers ; la fusion se fait alors à la main.

## À la main

```sh
git fetch upstream
git switch -c sync/upstream-$(date +%Y%m%d) origin/main
git merge upstream/main
# résoudre, puis :
cargo xtask ci
git push -u origin HEAD     # et ouvrir la pull request vers main
```

Pour résoudre :

- les catalogues `crates/ui-egui/src/i18n/*.tsv` se fusionnent seuls (union, `.gitattributes`) ;
- `Cargo.lock` : prendre celui de l'original (`git checkout --theirs Cargo.lock`), la compilation le complète ;
- `README.md` : garder la présentation de Plume, reprendre les chiffres et les nouveautés de l'original ;
- `ROADMAP.md`, `docs/*-parity.md` : garder les lignes des deux côtés dans les journaux et historiques ;
- le code : garder les deux changements ; l'original a souvent refactorisé (une fonction déplacée,
  un paramètre ajouté) et le code de Plume s'y adapte ; les tests nouveaux de l'original peuvent
  demander les champs que Plume a ajoutés (`Flow.comments`, `Ctx`…).

## Ce qui garde les fusions faciles

1. Les noms internes restent ceux de WordCraft (crates `wordcraft-*`, dossiers, identifiants) ;
   « Plume » ne passe que par `crates/brand` (nom, identifiant, icône, adresses).
2. Le code propre à Plume vit dans ses fichiers : `crates/diagram`, `crates/vcs`,
   `engine/src/plume.rs`, `cmd/md_blocks.rs`, `cmd/workflow.rs`, `io/textbundle.rs`,
   `ui-egui/src/diagram_*.rs`, `docs/plume/`. Les fichiers de l'original ne reçoivent que des
   points d'entrée courts.
3. Ce qui relève d'Office est **masqué**, jamais supprimé (`engine/src/plume_commands.txt`) : les
   correctifs de l'original continuent de s'appliquer sans conflit.
4. Ce qui sert aussi WordCraft (TextBundle, rendu Mermaid, correctifs) est proposé à l'original en
   pull request : l'écart se réduit d'autant.

## Historique

| Date | Fusion |
|---|---|
| 2026-10-20 | 280 commits de l'original fusionnés dans la branche de travail Plume : 36 fichiers en conflit, dont 22 catalogues (désormais en union), une dizaine de conflits de code résolus à la main. |
