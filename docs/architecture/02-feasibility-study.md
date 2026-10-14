# 02 — Étude de faisabilité

> Statut : audit initial, 14 octobre 2026. Légende : **[code]** vérifié dans le code · **[testé]** vérifié par un test exécuté · **[hypothèse]** à confirmer · **[prototype]** démontré par le prototype de cette branche.

## 1. Conclusion

**Option B retenue : adaptation de WordCraft avec couche documentaire spécifique.**

- L'interface, le moteur documentaire, la pagination, le PDF, les importateurs DOCX/DOC et le WASM sont conservés **sans modification**.
- La couche de transformation contrôlée existe déjà (`crates/formats/src/model.rs`, modèle `Flow`) ; on y ajoute un **profil Markdown natif** (zéro HTML, ressources externes, rapport d'adaptation). C'est fait dans cette branche (prototype, voir §4).
- Ce qui change : la frontière de persistance (`.md` devient le format d'enregistrement par défaut), quelques commandes d'édition à contraindre ou signaler, et l'ajout de deux briques neuves : **stockage/versionnement Git** et **connecteurs de forges**.

Pourquoi pas l'option A pure : le modèle en mémoire est bien plus riche que Markdown (couleurs, polices, objets flottants, sections…). Le sauvegarder « tel quel » en Markdown serait soit avec perte silencieuse (interdit par le cahier des charges), soit avec du HTML/XML caché (interdit). Une couche de profil avec rapport est nécessaire — d'où B.

Pourquoi pas l'option C : rien dans l'audit ne justifie de remplacer un composant. Le parseur Markdown existant est déjà CommonMark + GFM, borné et sans panique.

## 2. Réponses aux 12 questions de l'audit Markdown (§15.2)

| # | Question | Réponse | Source |
|---|---|---|---|
| 1 | Représentation en mémoire | `Document` riche (blocs `Para`/`Table`, runs + objets inline, parties secondaires, médias par clé) | `crates/doc/src/lib.rs:623` **[code]** |
| 2 | Import Markdown | `markdown::import` : parseur maison CommonMark (titres ATX/setext, listes imbriquées, citations, code, règles, tableaux GFM, emphase par l'algorithme des délimiteurs, liens, autoliens, entités, barré). Images : seules les URI `data:` devenaient des images. **Ajout : `import_with` résout les chemins relatifs** | `crates/formats/src/markdown.rs` **[code][prototype]** |
| 3 | Export Markdown | `markdown::export` via `model::from_doc`. Écrivait `<br>` dans les cellules, `<a id>` pour les signets et `![](image.png)` **sans écrire les fichiers images**. **Ajout : `export_native`** | idem **[code][prototype]** |
| 4 | Pertes à la conversion | soulignement, exposant/indice, couleurs, surlignage, polices/tailles, alignement, sauts de page, sections/en-têtes/pieds, notes de bas de page (texte du renvoi seul), cellules fusionnées, tableaux imbriqués, objets flottants (deviennent inline), graphiques/SmartArt (texte alternatif), équations (texte linéaire), suivi des modifications, champs (résultat seul) | `model.rs:803-960` **[code]** |
| 5 | Markdown format par défaut ? | Oui : changer `default_save_name` (`ui-egui/src/lib.rs:1547`) et traiter `.md` comme format « maître » (chemin + indicateur « modifié ») dans `cmd/file.rs:286-289` | **[code]** — à faire en phase 2 |
| 6 | Neutraliser les fonctions non représentables ? | Oui, par le champ `enabled` des `CommandSpec` (grisage selon un « mode Markdown » de la session) — sans retirer les commandes du ruban | `crates/engine/src/cmd/mod.rs` **[code]**, voir 03 §5 |
| 7 | Styles de rendu hors document ? | Oui : la feuille de styles `Document.styles` est reconstruite à l'import (`model::to_doc_in`) ; un **profil de publication** TOML peut fixer polices, tailles, marges à l'ouverture | **[hypothèse]** à prototyper (phase 3) |
| 8 | Images | `InlineObject::Image { media, w, h, alt }` + octets dans `Document.media`. Le profil natif les écrit dans `assets/img-<hash>.<ext>` | **[prototype]** |
| 9 | Liens relatifs | Les liens sont conservés tels quels (attribut de run) ; les images relatives sont désormais résolues à l'ouverture, **confinées au dossier du document** (`LocalImages`, protections `..`/liens symboliques/taille) | `crates/engine/src/io.rs` **[code][prototype]** |
| 10 | Tableaux sur plusieurs cycles | Oui pour les tableaux GFM simples : export → import → export identique. Les cellules multi-lignes sont aplaties **au premier cycle** puis stables | test `native_markdown_is_html_free_and_stable` **[testé]** |
| 11 | Conserver la pagination ? | Oui, intégralement : le Markdown est converti en `Document` puis mis en page par `layout` comme tout document | **[code]** |
| 12 | Éviter une modification profonde du moteur ? | Oui : aucune modification de `doc`, `layout`, `render`, `pdf`, `docx`, `docbin`. Modifications limitées à `formats/markdown.rs` et `engine/io.rs` | diff de cette branche **[code]** |

## 3. Tableau de faisabilité (§19)

| Fonction | Existant | Réutilisable | Modifications | Risque |
|---|---|---|---|---|
| Interface Word | Ruban complet, backstage, raccourcis, onglets contextuels — `crates/engine/src/ribbon.rs`, `crates/ui-egui/src/ribbon.rs`, `backstage.rs` | Oui, intégralement | Ajouter un onglet **Publication**, un groupe **Versions** dans Révision, un panneau **Historique** ; griser les commandes non représentables en mode Markdown | Faible |
| Moteur documentaire | `crates/doc` (modèle), `crates/engine` (Session, undo, 500 niveaux, commandes) | Oui | Aucune dans `doc` ; dans `engine` : notion de « format maître » et mode Markdown | Faible |
| Markdown | `crates/formats/src/markdown.rs` (import/export CommonMark+GFM), `model.rs` (Flow) | Oui | **Fait :** `export_native`, `import_with`, `html_lines`, rapport. **À faire :** notes de bas de page GFM `[^n]`, commentaires ancrés, front matter YAML, listes de tâches à l'export | Moyen (fidélité) |
| DOCX | `crates/docx` (lecture/écriture OOXML, chiffrement) | Oui, comme importateur | Aucune | Faible |
| DOC ancien | `crates/docbin` (Word 97-2003, [MS-DOC], lecture seule) | Oui | **Word 6.0/95 refusé** (`docbin/src/fib.rs:117`) : à documenter, éventuel ajout ultérieur | Moyen (Word 95 et antérieurs) |
| PDF | `crates/pdf` (krilla : polices embarquées, liens, signets des titres, PDF balisé) | Oui, intégralement | Aucune ; appliquer le profil de publication avant l'export | Faible |
| Pagination | `crates/layout` | Oui | Aucune | Faible |
| Images | `Document.media`, `InlineObject::Image`, `LocalImages` (HTML) | Oui | **Fait :** extraction `assets/`, noms stables, dédoublonnage, réimport. **À faire :** taille d'affichage (perdue en GFM pur), images non web (EMF/WMF/TIFF → PNG) | Moyen |
| Git local | Rien (versions en mémoire uniquement : `cmd/tools.rs:396`) | Le modèle de commandes `file.versions` | Nouvelle crate `vcs` (L2/L4) : historique, commit, diff, restauration — voir 05 | Moyen |
| GitHub | Rien | — | Connecteur API REST (contenu, commits, PR, commentaires) + OAuth device flow | Moyen |
| GitLab | Rien | — | Idem API v4 | Moyen |
| Forgejo | Rien | — | Idem API (compatible Gitea) ; le porteur a déjà un fork `forgejo-mcp` | Moyen |
| WebAssembly | `apps/wordcraft-web`, toutes les crates vérifiées par `cargo xtask wasm` | Oui | Pas de système de fichiers ni de Git natif dans le navigateur : utiliser les **API REST des forges** (pas de clone) et un stockage IndexedDB/OPFS | Élevé (Git + CORS + auth) |

## 4. Prototype réalisé dans cette branche

Jalon visé (§21) :

```text
Ouverture DOCX → édition → sauvegarde Markdown → fermeture → réouverture Markdown → édition sans perte → export PDF
```

Ce qui est démontré **par un test automatique** (`crates/engine/src/tests.rs::docx_to_native_markdown_round_trip`) :

1. un document (document d'exemple WordCraft + une image) est écrit en DOCX puis rouvert via l'importateur DOCX ;
2. `io::save_markdown` écrit `doc.md` + `assets/img-<hash>.png` ;
3. le fichier ne contient **aucune ligne HTML** (`markdown::html_lines`) ;
4. la réouverture recharge l'image depuis `assets/` ; un nouvel enregistrement produit **exactement le même texte** ;
5. une édition par la couche de commandes (`text.insert`) est enregistrée, relue, et le reste du fichier n'a pas bougé ;
6. l'export PDF du document rouvert produit un PDF valide.

Ce qui reste **non démontré** : l'ergonomie dans l'interface graphique (Enregistrer → `.md` par défaut), le rapport d'import affiché à l'utilisateur, le corpus réel de documents Word du porteur.

## 5. Points de vigilance

1. **Le rendu « visuel » cible** (GitHub, VitePress, VitePress + plugins) change ce qui est représentable : notes de bas de page, conteneurs `::: tip`, ancres `{#id}`, mathématiques. Voir 03 §3 — c'est le principal choix de produit restant.
2. **Commentaires** : la seule exception au zéro HTML (commentaires HTML `<!-- -->`). Le prototype les conserve en bloc en fin de fichier ; l'ancrage précis est conçu en 03 §6.
3. **Word 95 et antérieurs** : non pris en charge par `docbin`. « N'importe quel Word » nécessite soit un nouvel importateur, soit un convertisseur externe facultatif.
4. **Scripts de conversion existants du porteur** (`convertDoC`) : non consultés — l'accès à ce dépôt n'a pas été accordé à cette session. À comparer avec l'importateur WordCraft dès que l'accès est donné.
