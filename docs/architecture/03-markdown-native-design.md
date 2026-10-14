# 03 — Conception du format natif Markdown

> Statut : proposition + premier incrément implémenté (`export_native`, `import_with`, `html_lines`, `io::save_markdown`).

## 1. Principe

> L'utilisateur édite visuellement un `Document` WordCraft complet ; **à l'enregistrement**, le document est projeté sur un **profil Markdown** ; **à l'ouverture**, le Markdown est reconstruit en `Document` avec la feuille de styles du **thème visuel** choisi.

```text
              ┌──────────── édition WYSIWYG (inchangée) ────────────┐
.docx/.doc ──►│  Document (crates/doc)  ──►  layout ──► écran / PDF  │
              └──────────────┬───────────────────────▲──────────────┘
                     export_native              import_with
                (profil + rapport)          (assets + thème)
                             ▼                       │
                     document.md  +  assets/img-<hash>.<ext>
```

Pas de conversion à chaque frappe : la projection n'a lieu qu'à l'enregistrement (et, plus tard, pour l'indicateur « éléments non conservés », de façon différée).

## 2. Règles absolues du fichier `.md`

1. **Zéro HTML.** Seule exception : les **commentaires HTML** `<!-- … -->`, invisibles dans tous les moteurs de rendu, réservés aux commentaires de relecture. Vérifié par `markdown::html_lines` (tests).
2. **Aucune donnée de présentation encodée** (pas de XML, pas d'attributs de style, pas de base64).
3. **Images externes**, en chemins relatifs : `assets/img-<hash FNV-64>.<ext>`. Le nom dépend uniquement des octets : réenregistrer ne renomme rien, deux images identiques sont stockées une fois, et un diff Git sur le `.md` ne bouge pas quand on ne touche pas aux images.
4. **Idempotence** : `export(import(export(d))) == export(d)`. Les pertes éventuelles ont lieu **une seule fois**, au premier enregistrement, et sont signalées.
5. **Lisible et modifiable à la main** : un `.md` modifié dans VS Code ou sur GitHub se rouvre normalement.

## 3. Profils de rendu cibles (« cible visuelle »)

Le porteur souhaite pouvoir choisir la cible visuelle : **GitHub**, **VitePress par défaut**, **VitePress avec plugins étendus**. Chaque profil fixe (a) les constructions autorisées dans le fichier, (b) le thème visuel de l'éditeur (polices, tailles des titres, style des tableaux et du code calqués sur la cible, **réalisés avec nos propres couleurs et polices libres** — aucune ressource GitHub copiée).

| Construction | `gfm` (GitHub, GitLab, Forgejo) | `vitepress` (défaut) | `vitepress-ext` (plugins) |
|---|---|---|---|
| Titres, gras, italique, barré, code, citations, listes, règles | oui | oui | oui |
| Tableaux pipe, alignement de colonnes | oui | oui | oui |
| Listes de tâches `- [ ]` | oui | oui | oui |
| Notes de bas de page `[^1]` | **oui** | non → note convertie en texte « (note : …) » | oui (`markdown-it-footnote`) |
| Encadrés | `> [!NOTE]` (alertes GitHub) | `> [!NOTE]` (pris en charge par VitePress) | `::: tip` / `::: warning` |
| Ancre explicite de titre | non (slug automatique) | `## Titre {#ancre}` | `{#ancre}` |
| Table des matières | non (générée par le site) | `[[toc]]` | `[[toc]]` |
| Mathématiques | `$…$`, `$$…$$` | non → texte linéaire | `$…$` (option `math`) |
| Diagrammes | bloc ` ```mermaid ` | non | ` ```mermaid ` (plugin) |
| Front matter YAML | affiché en tableau par GitHub | oui (titre, description) | oui |

**Recommandation :** `gfm` par défaut (c'est le seul rendu commun à GitHub, GitLab et Forgejo, donc à la collaboration via forge) ; le profil est une propriété **du dépôt / du dossier** (fichier `publication/default.toml`, clé `profile`), pas du document — ainsi tous les documents d'un même site se comportent pareil.

Le prototype implémente le **socle commun** des trois profils (sans notes, encadrés ni mathématiques), qui est valide partout.

## 4. Correspondance Word → Markdown (profil `gfm`)

| Élément Word / WordCraft | Écriture | Traitement | Rapport (`Adaptation`) |
|---|---|---|---|
| Titre 1-6, Titre du document | `#`…`######` | conservé | — |
| Gras, italique, barré | `**`, `*`, `~~` | conservé | — |
| Police à chasse fixe | `` `code` `` | conservé | — |
| Style « Code » | bloc ` ``` ` | conservé | — |
| Citation / Citation intense | `>` | conservé | — |
| Listes à puces / numérotées, 9 niveaux | `-`, `1.` indentation 4 | conservé (le type de numérotation i, a, A… est normalisé en 1.) | — |
| Lien externe | `[texte](url)` | conservé ; les schémas dangereux (`javascript:`…) sont écrits en texte | — |
| Lien interne vers un titre | `[texte](#slug)` | **à faire** : réécrire `#_Toc123` vers le slug du titre | — |
| Signet hors titre | — | supprimé | `Bookmark` |
| Image inline ou flottante | `![alt](assets/img-….png)` | fichier externe, flottant → inline ; **taille d'affichage perdue** en GFM pur (l'image s'affiche à sa taille en pixels, réduite à la largeur de page) | — (taille : à ajouter) |
| Graphique, SmartArt, forme | `*[texte alternatif]*` | **à faire** : rendre en PNG via `render` et écrire comme image | `Figure` |
| Équation | texte linéaire | **à faire** : `$…$` en `gfm` | `Equation` |
| Tableau simple | tableau pipe | conservé, 1re ligne = en-tête | — |
| Cellule à plusieurs paragraphes | paragraphes joints par une espace | aplati | `CellParagraphs` |
| Cellule fusionnée | cellules vides | aplati | `MergedCells` |
| Tableau imbriqué | texte dans la cellule | aplati | `NestedTable` |
| Tableau de mise en page (sans bordure, sert à positionner) | **à faire** : dérouler en paragraphes | — | — |
| Soulignement | texte simple | perdu | `Underline` |
| Exposant / indice | texte simple | perdu (Unicode ¹²³ possible pour les chiffres : **à étudier**) | `SupSub` |
| Couleur, surlignage | — | déporté vers le profil de publication | `Color` |
| Police, taille | — | normalisé par le thème (non signalé : attendu) | — |
| Centré / droite | — | déporté vers le profil | `Alignment` |
| Justifié | — | normalisé par le thème (non signalé) | — |
| Saut de page | — | **à étudier** : règle `---` + profil « saut avant H1 » | `PageBreak` |
| En-têtes, pieds, numéros de page, marges, orientation | — | profil de publication (`publication/default.toml`) | — |
| Notes de bas de page | aujourd'hui : numéro de renvoi seul | **à faire** : `[^n]` + définitions en fin de fichier | — |
| Commentaires | commentaires HTML | voir §6 | compteur `comments` |
| Suivi des modifications en attente | — | voir §7 | — |
| Champs (TOC, PAGE, REF…) | résultat texte | TOC : supprimée (le site la génère) ou `[[toc]]` | **à faire** |

## 5. Commandes d'édition en mode Markdown

Les commandes existent toutes déjà dans le registre (`crates/engine/src/cmd/*.rs`). Trois traitements, choisis commande par commande via le champ `enabled` des `CommandSpec`, **sans retirer aucun bouton du ruban** :

1. **Conservées** (représentables) : titres, styles de paragraphe compatibles, gras/italique/barré, listes, liens, images, tableaux, citations, code, recherche/remplacement, orthographe, annuler/rétablir, zoom, navigation.
2. **Normalisées** (acceptées, le résultat dépend du profil) : polices, tailles, justification, interligne, espacements. Elles modifient l'affichage pendant la session ; à l'enregistrement le thème du profil s'applique.
3. **Signalées** (non représentables) : couleur, soulignement, exposant/indice, zones de texte, WordArt, formes, colonnes, sections, en-têtes/pieds dans le corps, cellules fusionnées. Deux modes au choix de l'utilisateur :
   - **souple** (défaut) : la commande s'exécute, la barre d'état affiche « 3 mises en forme ne seront pas conservées » (alimenté par `Report`) ;
   - **strict** : la commande est grisée avec une info-bulle explicative.

Le mode est une option de session (`Session`), il n'affecte pas les documents ouverts en `.docx` (le format Word reste disponible en **export**).

## 6. Commentaires (seule exception au zéro HTML)

**Incrément actuel** (prototype) : les commentaires du document et leurs réponses sont écrits en fin de fichier dans un unique commentaire HTML :

```markdown
<!-- wordcraft:comments
1. Alice on “le budget prévisionnel”: chiffre à vérifier
   - Bob: vérifié avec la compta
-->
```

**Cible** (phase 5) : ancrage précis + fil, toujours invisible au rendu.

```markdown
Le <!-- c1 -->budget prévisionnel<!-- /c1 --> est validé.

<!-- wordcraft:comments
c1 | Alice | 2026-10-14T09:12Z | ouvert
   chiffre à vérifier
c1.1 | Bob | 2026-10-14T10:03Z
   vérifié avec la compta
-->
```

- À l'ouverture, les paires `<!-- cN -->…<!-- /cN -->` redeviennent des `CommentStart/End` et le bloc final reconstruit `Document.comments` → les commentaires s'affichent en bulles comme dans Word, et toutes les commandes `review.*Comment` fonctionnent.
- En collaboration via une forge, les commentaires peuvent **aussi** venir des revues de demande de fusion (voir 05 §6) : ils sont alors affichés comme commentaires Word mais restent stockés côté forge.
- Contraintes : `--` est interdit dans un commentaire HTML (remplacé), un marqueur d'ancrage ne doit pas couper une construction Markdown (placement aux limites de mots, hors code).

## 7. Suivi des modifications

Markdown n'a pas de suivi des modifications : **le suivi devient l'historique Git**.

- Pendant l'édition, le suivi WordCraft (Ctrl+Maj+E) reste utilisable : insertions/suppressions colorées par auteur.
- À l'enregistrement d'un document avec des révisions en attente, le fichier contient **la version avec les modifications acceptées** et l'utilisateur est prévenu ; l'état « avec révisions » est conservé dans la version Git précédente.
- « Afficher les modifications » d'une version Git = **Comparer** (`review.compare`, déjà implémenté) entre la version N-1 et N : le résultat est un document avec révisions attribuées à l'auteur du commit, et **Accepter / Rejeter** fonctionnent tels quels.

## 8. Ressources

- Dossier `assets/` à côté du `.md` (paramétrable : `NativeOptions::assets_dir`). Dans un dépôt de plusieurs documents, un dossier `assets/` partagé est sans risque grâce au nommage par contenu.
- Un fichier déjà présent avec le même nom n'est pas réécrit (même nom ⇒ mêmes octets, au risque de collision FNV près — négligeable à cette échelle, et sans conséquence de sécurité : le nommage ne sert pas à l'intégrité).
- Les images orphelines (plus référencées) ne sont **pas** supprimées automatiquement : **à faire**, nettoyage proposé à l'enregistrement d'une version.
- Formats non web (EMF, WMF, TIFF, BMP) : **à faire**, conversion PNG à l'enregistrement (le moteur sait déjà les dessiner).
- À l'ouverture : seules les images **à l'intérieur du dossier du document** sont lues (`LocalImages` : pas de `..`, pas d'URL, pas de chemin absolu, liens symboliques résolus, plafond de 256 Mo par fichier et 1 Go par document).

## 9. API implémentée (incrément 1)

`crates/formats/src/markdown.rs` :

| Élément | Rôle |
|---|---|
| `export_native(&Document, &NativeOptions) -> Bundle` | texte + `assets: Vec<(chemin, octets)>` + `Report` |
| `NativeOptions { assets_dir, comments }` | défaut `assets`, commentaires conservés |
| `Report { counts: BTreeMap<Adaptation, usize>, images, comments }`, `lines()`, `is_lossless()` | rapport d'adaptation |
| `Adaptation` | `Underline`, `SupSub`, `Color`, `Alignment`, `PageBreak`, `Bookmark`, `CellParagraphs`, `NestedTable`, `MergedCells`, `Figure`, `Equation` |
| `import_with(text, page, resolve)` | import avec résolution des images relatives (deux passes, état thread-local remis à zéro par garde) |
| `html_lines(&str) -> Vec<usize>` | lignes contenant du HTML hors commentaires, code et autoliens |

`crates/engine/src/io.rs` :

| Élément | Rôle |
|---|---|
| `save_markdown(path, doc) -> Report` | écrit le `.md` et `assets/` (écritures atomiques, racine d'automatisation respectée) |
| `save_path_with` | un chemin `.md`/`.markdown` passe désormais par `save_markdown` |
| `open_path_in` | un `.md` ouvert depuis le disque résout ses images relatives |

`markdown::export` (copie « historique » utilisée par `save_bytes`, le Web et la CLI vers la sortie standard) est **inchangé** octet pour octet ; ses tests passent.
