# Plume et le Markdown : audit

> **Date :** 2026-10-18 · **Statut :** audit, décisions prises le 2026-10-18 (section 10) · **Base :** branche `claude/gallant-allen-2rb6nx`, commit `5030980`

## 1. En bref

Plume écrit déjà un Markdown propre pour le cœur du langage : titres, emphase, listes, tâches, tableaux, notes, liens, images, maths, alertes GitHub simples. En revanche, **il détruit aujourd'hui une partie des fichiers Markdown qu'il n'a pas écrits lui-même**. Un fichier VitePress ou GitHub ordinaire, ouvert puis enregistré, perd des blocs de code, des conteneurs, des images et des inclusions. C'est le premier chantier, avant toute nouvelle interface.

Les cinq constats principaux :

1. **Pertes de données à l'ouverture.** Six défauts corrompent un fichier existant (section 2.2). Le pire : deux blocs de code qui se suivent fusionnent, et le langage du bloc disparaît, donc un diagramme `mermaid` devient du code ordinaire collé au bloc précédent.
2. **Syntaxes étendues absentes de l'interface.** Alertes, conteneurs VitePress, langage des blocs de code, diagrammes, ancres de titre et inclusions n'ont aucune représentation dans l'éditeur. La section 3 propose une représentation pour chacune.
3. **Le nom du document est celui du fichier.** La barre de titre et la liste des récents montrent le nom du fichier et son chemin, jamais le titre du document. La section 4 propose de suivre Office : le titre du front matter d'abord.
4. **Les métadonnées Word sont perdues en silence.** Lors d'une conversion DOCX vers Markdown, seuls le titre, l'auteur, le sujet, les mots clés et le résumé passent. La catégorie, les dates, la révision, la société et toutes les propriétés personnalisées, comme un champ `toto`, disparaissent sans figurer dans le rapport. La section 5 propose la correspondance et l'interface d'édition.
5. **Les dessins peuvent devenir du Markdown pur.** SmartArt, graphiques et organigrammes faits de formes reliées ont une structure que Plume connaît déjà. Ils peuvent devenir des diagrammes Mermaid textuels au lieu d'images. La section 7 détaille ce qui est faisable.

## 2. Ce qui est lu, écrit et montré

### 2.1 État par syntaxe

Mesuré avec un fichier d'essai qui contient toutes les syntaxes, ouvert puis enregistré par Plume.

| Syntaxe | Cible | Lu | Réécrit fidèlement | Visible dans l'éditeur |
|---|---|---|---|---|
| Titres, gras, italique, barré, code en ligne | toutes | oui | oui | oui |
| Saut de ligne dur `\` | toutes | oui | oui | oui |
| Listes, listes de tâches `- [ ]` | toutes | oui | oui | oui, cases ☐ ☒ |
| Tableaux, alignement des colonnes | toutes | oui | oui, délimiteurs normalisés | oui |
| Notes de bas de page `[^1]` | GitHub, VitePress étendu | oui | oui | oui |
| Maths `$…$`, `$$…$$` | GitHub, VitePress étendu | oui | oui, bloc remis sur une ligne | oui, éditeur d'équations |
| Front matter YAML | toutes | oui | clés connues réécrites, autres clés conservées | 5 champs seulement, dans Fichier › Informations |
| Alertes `> [!NOTE]` | GitHub, VitePress | oui, comme citation avec titre en gras | **non si deux alertes se suivent** | comme une citation |
| Conteneurs `::: tip`, `::: details` | VitePress | **non** | **non, fusionnés en un paragraphe** | non |
| Groupes de code `::: code-group` | VitePress | **non** | **non** | non |
| Langage d'un bloc de code ` ```js ` | toutes | **non** | **non, langage perdu** | non |
| Lignes surlignées ` ```js{2} ` | VitePress | **non** | **non** | non |
| Diagrammes ` ```mermaid ` | GitHub, VitePress étendu | **non** | **non, fusionné au bloc voisin** | non |
| Ancre de titre `{#id}` | VitePress | comme texte | oui, comme texte | comme texte dans le titre |
| Table des matières `[[toc]]` | VitePress | comme texte | **non, réécrit `\[\[toc\]\]`** | comme texte |
| Inclusion `<!--@include: …-->` | VitePress | **non, ignorée** | **non, supprimée** | non |
| Image distante ou introuvable | toutes | **non** | **non, remplacée par son texte alternatif** | texte seul |
| Titre d'image `"titre"` | toutes | non | non | non |
| HTML brut (`<kbd>`, `<details>`) | GitHub, VitePress | comme texte | **non, échappé** | comme texte |
| Indice et exposant `H~2~O`, `x^2^` | VitePress étendu | comme texte | indice échappé | comme texte |
| Surlignage `==texte==` | VitePress étendu | comme texte | oui, comme texte | comme texte |
| Émojis `:tada:` | GitHub, VitePress | comme texte | oui | comme texte |
| Listes de définitions | VitePress étendu | non | **non, ligne fusionnée** | non |
| Clés `tags` en liste YAML | toutes | réinterprétées en `keywords` | **non, clé renommée** | dans Mots clés |

### 2.2 Les six pertes de données à corriger d'abord

| Rang | Défaut | Effet sur un fichier existant |
|---|---|---|
| 1 | Le langage d'un bloc de code est ignoré | Un bloc `mermaid` ou `js` perd son langage et sa coloration |
| 2 | Deux blocs de code qui se suivent fusionnent | Le code de deux blocs, ou un diagramme et un exemple, devient un seul bloc |
| 3 | Les conteneurs `:::` sont lus comme du texte | `::: tip`, son contenu et `:::` deviennent une seule ligne de texte |
| 4 | Une image introuvable devient son texte alternatif | L'URL de l'image disparaît à l'enregistrement |
| 5 | Les commentaires HTML sont supprimés | Les directives `<!--@include-->` et les commentaires de l'auteur disparaissent |
| 6 | Deux alertes qui se suivent fusionnent | La seconde alerte entre dans la première |

Règle proposée : **ce que Plume ne comprend pas, il le garde tel quel**. Un bloc inconnu devient un bloc « source brute » non interprété, visible et éditable comme du code, et réécrit à l'identique.

## 3. Représenter les syntaxes étendues dans l'interface

Le principe : chaque construction devient un objet que l'on voit et que l'on modifie sans connaître la syntaxe. Le profil de publication choisit ensuite la syntaxe écrite. Aujourd'hui, les profils `gfm` et `vitepress` ne changent que l'apparence ; il faut y ajouter `vitepress-ext` et leur faire choisir la syntaxe.

| Construction | Dans l'éditeur | Pour l'insérer | Écrit en `gfm` | Écrit en `vitepress` |
|---|---|---|---|---|
| Encadré : note, astuce, important, avertissement, attention | Bloc avec barre de couleur à gauche, icône et titre éditable | Insertion › Encadré, et la galerie de styles | `> [!NOTE]` | `::: tip Titre` |
| Bloc repliable | Encadré avec un triangle et un titre | Insertion › Encadré › Détails | Encadré `> [!NOTE]` avec titre, signalé | `::: details Titre` |
| Bloc de code | Fond gris, étiquette du langage en haut à droite, coloration | Insertion › Bloc de code, liste des langages | ` ```js ` | ` ```js{2} ` |
| Groupe de code | Onglets au-dessus des blocs | Insertion › Bloc de code › Groupe d'onglets | Blocs à la suite, avec leur titre en commentaire | `::: code-group` |
| Diagramme Mermaid | Le diagramme dessiné ; un clic montre la source à côté, avec aperçu en direct | Insertion › Diagramme : séquence, flux, Gantt, classes, états, camembert, carte mentale | ` ```mermaid ` | ` ```mermaid `, profil étendu |
| Équation | Comme aujourd'hui | Comme aujourd'hui | `$…$` | `$…$`, profil étendu |
| Ancre de titre | Petite étiquette `#ancre` grisée en fin de titre | Clic droit sur un titre › Ancre | Rien, ancre automatique | `{#ancre}` |
| Table des matières | Le bloc de table des matières actuel | Références › Table des matières | Liste de liens | `[[toc]]` |
| Inclusion | Pastille « Inclus : autre.md », non éditable, ouvre le fichier | Insertion › Inclure un fichier | Lien vers le fichier | `<!--@include: ./autre.md-->` |
| HTML brut | Bloc « HTML » grisé, réécrit à l'identique | Non proposé | Inchangé | Inchangé |
| Surlignage, indice, exposant | Mise en forme habituelle | Accueil › Police | Retiré, signalé | `==…==`, `~…~`, `^…^`, profil étendu |

Deux réglages complètent l'ensemble :

- **Le profil choisit ce qui est proposé.** En `gfm`, les boutons des constructions que GitHub n'affiche pas sont masqués, comme le fait déjà le profil Plume pour Office.
- **Un mode « source » par bloc.** Tout bloc spécial s'ouvre en source Markdown d'un clic, pour l'utilisateur qui connaît la syntaxe.

## 4. Nom du document, ouverture et fermeture

### Ce qui se passe aujourd'hui

- La barre de titre affiche le nom du fichier sans extension, par exemple `guide-installation`. Le titre du front matter n'est utilisé que pour un document jamais enregistré.
- La liste des récents affiche le nom du fichier et, en dessous, son chemin complet.
- Un document ouvert depuis une forge n'entre pas dans les récents.
- Fichier › Informations édite 5 propriétés seulement : titre, sujet, auteur, mots clés et catégorie. La catégorie n'est pas enregistrée en Markdown.

### Ce qui est proposé, sur le modèle d'Office

| Moment | Proposition |
|---|---|
| Barre de titre | Le `title` du front matter, sinon le nom du fichier. Le nom du fichier s'affiche en infobulle. |
| Nouveau document | Le titre est demandé à la première sauvegarde. Il va dans `title`, et le nom de fichier proposé en dérive : `Guide d'installation` donne `guide-d-installation.md`. |
| Récents | Titre en gras, puis lieu : dossier, ou `forge · dépôt · branche`, et date d'ouverture. Les documents de forge y figurent. |
| Récents, recherche | Filtre par titre, par fichier ou par dépôt. |
| Fermeture | La question « Enregistrer les modifications de *Titre* ? » nomme le titre. Sur un dépôt, un troisième choix : « Enregistrer une version ». |
| Renommer | Changer le titre propose de renommer le fichier, sans l'imposer : les liens vers le fichier en dépendent. |

## 5. Métadonnées : de Word au front matter

### 5.1 Ce que devient chaque propriété Word aujourd'hui

| Propriété Word | Aujourd'hui | Clé proposée | Lue par |
|---|---|---|---|
| Titre | `title` | `title` | pandoc, Eisvogel, VitePress, GitHub |
| Sujet | `subject` | `subtitle` | pandoc, Eisvogel |
| Auteur | `author` | `author` | pandoc, Eisvogel |
| Mots clés | `keywords`, une chaîne | `keywords`, une liste | pandoc, Eisvogel |
| Commentaires | `description` | `description` | VitePress, pandoc |
| Catégorie | **perdue** | `category` | Plume, sites |
| Créé le | **perdue en conversion DOCX** | `date` | pandoc, Eisvogel |
| Modifié le | **perdue** | `lastUpdated` | VitePress ; la date git sinon |
| Dernier modifié par | **perdue** | `editor` | Plume |
| Révision | **perdue** | `version` | Plume, modèles PDF |
| Société | **perdue** | `publisher` | pandoc |
| Responsable | **perdue** | `manager` | Plume |
| Statut | **perdue** | `status` | Plume |
| Logiciel | non écrit | `generator: Plume 0.7` | Plume |
| Propriété personnalisée `toto` | **perdue** | voir 5.2 | pandoc, modèles PDF |

### 5.2 Les propriétés personnalisées

Il n'existe pas de norme : chaque outil lit les clés qu'il connaît et ignore les autres. GitHub affiche toutes les clés dans un tableau en tête de fichier. Pandoc et les modèles Eisvogel peuvent utiliser n'importe quelle clé, sous la forme `$toto$`. VitePress ignore les clés inconnues.

Deux placements sont possibles, à trancher (question 3) :

```yaml
# A. À la racine : simple, lisible, utilisable telle quelle dans un modèle PDF
toto: valeur perso
client: ACME

# B. Sous une clé dédiée : aucun risque de collision avec une clé de VitePress
properties:
  toto: valeur perso
  client: ACME
```

Les propriétés techniques de Word, comme les préférences Zotero, les étiquettes de confidentialité `MSIP_Label_…` ou « Marquer comme final », n'ont pas de sens hors de Word. Elles sont listées dans le rapport de conversion et ne sont pas écrites.

### 5.3 Modifier les métadonnées depuis le document

Proposition : un volet **Propriétés du document**, ouvert depuis Fichier › Informations et depuis la barre d'état.

- **Champs standards** en haut : titre, sous-titre, auteurs, date, version, statut, mots clés sous forme d'étiquettes, résumé.
- **Champs personnalisés** en dessous, ajoutés ou retirés avec + et −, comme les propriétés personnalisées de Word.
- **Vue YAML** brute en bas, éditable, pour les réglages Eisvogel et VitePress.
- **Modèles de front matter**, choisis à la création ou dans le volet. Chacun préremplit les champs utiles :

| Modèle | Champs |
|---|---|
| Note | `title`, `author`, `date`, `tags` |
| Rapport | `title`, `subtitle`, `author`, `date`, `version`, `status`, `publisher`, `keywords`, `abstract`, réglages Eisvogel |
| Documentation technique | `title`, `description`, `version`, `status`, `editor`, `lastUpdated` |
| Page VitePress | `title`, `description`, `outline`, `layout`, `prev`, `next` |
| Article | `title`, `author`, `date`, `abstract`, `keywords`, `lang`, `bibliography` |

## 6. Audit des menus

Le profil Plume masque déjà 309 commandes Office sur 662. Le contrôle a porté sur les 353 commandes restantes : l'effet de chacune survit-il à l'enregistrement en Markdown ?

### 6.1 Classées « Markdown » à tort : effet perdu à l'enregistrement

| Commande | Effet perdu | Proposition |
|---|---|---|
| `insert.bookmark`, `insert.deleteBookmark` | Le signet disparaît | Remplacer par « Ancre de titre » (section 3) |
| `para.indent`, `para.outdent` hors liste | Le retrait disparaît | Garder dans les listes seulement |
| `para.rtl`, `para.ltr` | Le sens du texte disparaît | Masquer |
| `para.align` avec centré ou droite | L'alignement disparaît | Masquer ces valeurs |
| `para.defineNumber`, `para.multilevel`, `para.restartNumbering`, `list.define` | Le style de numérotation devient `1.` | Garder puces et numéros simples |
| `styles.create`, `styles.modify`, `styles.organizer`, `styles.copyFrom`, `styles.updateToMatch`, `styles.rename`, `styles.delete` | Un style créé ou modifié ne survit pas | Remplacer par le thème du profil de publication |
| `picture.size` | La taille de l'image disparaît | Masquer en `gfm` ; en `vitepress-ext`, écrire `{width=…}` |
| `table.repeatHeader` | Sans objet : la première ligne est toujours l'en-tête | Masquer |
| `references.updateFields`, `insert.dateTime` en champ | Le champ devient du texte figé | Garder l'insertion en texte seulement |
| `tools.templates` | Modèles Word `.dotx` | Remplacer par les modèles de front matter |
| `file.privacyOptions`, `file.privacy` | Réglages propres au DOCX | Masquer |
| `review.trackChanges` sans version | Déjà refusé par le profil | — |

### 6.2 Utiles et absentes

| Manque | Proposition |
|---|---|
| Insérer un encadré | Insertion › Encadré |
| Insérer un bloc de code avec langage | Insertion › Bloc de code |
| Insérer un diagramme | Insertion › Diagramme |
| Insérer une inclusion | Insertion › Inclure un fichier |
| Ancre d'un titre | Clic droit sur un titre |
| Propriétés personnalisées | Volet Propriétés du document |
| Choisir le profil de publication | Fichier › Options › Markdown, et la barre d'état |
| Voir la source Markdown | Affichage › Source, en vis-à-vis |

## 7. Dessins : vers du Markdown pur

Le principe est le même que pour le reste : la forme textuelle d'abord, l'image seulement s'il n'y en a aucune.

| Objet Word | Ce que Plume sait de lui | Conversion proposée | Fidélité |
|---|---|---|---|
| SmartArt processus | Étapes et ordre | Mermaid `flowchart LR` : A → B → C | Bonne |
| SmartArt cycle | Étapes et ordre | Mermaid `flowchart` qui reboucle | Bonne |
| SmartArt hiérarchie | Arbre des éléments | Mermaid `flowchart TD`, ou `mindmap` | Bonne |
| SmartArt radial | Centre et branches | Mermaid `mindmap` | Bonne |
| SmartArt liste, pyramide, matrice, Venn | Éléments et niveaux | Liste à puces | Le texte reste, la forme se perd |
| Graphique camembert ou anneau | Catégories et valeurs | Mermaid `pie`, plus un tableau des données | Bonne |
| Graphique colonnes, barres, lignes | Séries et valeurs | Mermaid `xychart-beta`, plus un tableau des données | Bonne, sans les empilements |
| Formes reliées par des connecteurs | Texte des formes, origine et cible des connecteurs, flèches | Mermaid `flowchart` | Bonne si les connecteurs sont collés aux formes |
| Formes posées sans connecteurs | Texte et position | Texte transcrit, comme aujourd'hui | Le texte reste |
| Diagramme de séquence dessiné à la main | Des formes et des traits, sans le sens | Proposition par l'IA locale, à valider | Variable |
| Encre, dessin libre, image | Aucune structure | Image PNG | Image |

**Dans Plume lui-même, on ne dessine plus à main levée.** On écrit un diagramme Mermaid avec un aperçu en direct, à partir de modèles. Des moteurs Mermaid en Rust pur existent et produisent du SVG sans navigateur : `mermaid-rs-renderer`, `mermaid-svg`, `flowmaid`. Il faut en choisir un après vérification de sa licence, de sa maintenance et des types de diagrammes pris en charge. L'export PDF dessine alors le diagramme, et GitHub l'affiche nativement.

## 8. Tableaux

Contrôle par un tableau à en-têtes sur deux niveaux, une cellule fusionnée verticalement, une ligne vide, des cellules à plusieurs paragraphes, une liste, du code, une barre verticale et un tableau imbriqué.

| Cas | Résultat | Verdict |
|---|---|---|
| En-têtes sur deux niveaux | `2025 – T1`, `2025 – T2`… | Bon |
| Fusion verticale | `Poires` répété sur chaque ligne | Bon |
| Ligne entièrement vide | Supprimée | Bon |
| Barre verticale dans le texte | `A \| B` | Bon |
| Code dans une cellule | `` `x = 1` `` | Bon |
| En-tête en gras | `**Produit**` alors que l'en-tête est déjà en gras | À corriger : bruit inutile |
| Plusieurs paragraphes dans une cellule | `Premier paragraphe. Second paragraphe.` | À corriger : la séparation disparaît |
| Liste dans une cellule | `un deux` | **À corriger : les puces disparaissent** |
| Tableau imbriqué | `i1 i2 i3 i4` | **À corriger : la structure disparaît** |

Corrections proposées, sans HTML :

- En-tête : retirer le gras quand toute la cellule d'en-tête est en gras.
- Paragraphes : les séparer par ` · `, ou par `<br>` si le profil autorise ce seul élément HTML.
- Listes : écrire `• un · • deux`, ou `- un<br>- deux` avec `<br>`.
- Tableau imbriqué : l'écrire juste après le tableau parent, avec un renvoi dans la cellule, par exemple « voir tableau 1a ». La restructuration par IA reste proposée pour ces cas.

## 9. Plan de travail proposé

| Lot | Contenu | Priorité |
|---|---|---|
| 1 | Les six pertes de données, avec un test de stabilité sur un fichier VitePress complet | Immédiate |
| 2 | Métadonnées Word complètes vers le front matter, et volet Propriétés du document | Haute |
| 3 | Titre du document dans la barre de titre, les récents et la fermeture | Haute |
| 4 | Blocs de code avec langage, encadrés, profil `vitepress-ext` | Haute |
| 5 | Diagrammes Mermaid : rendu, insertion, conversion des SmartArt, graphiques et organigrammes | Moyenne |
| 6 | Reclassement des commandes de la section 6.1 et nouvelles commandes de la section 6.2 | Moyenne |
| 7 | Corrections des tableaux de la section 8 | Moyenne |
| 8 | Inclusions, ancres de titre, vue source en vis-à-vis | Basse |

## 10. Décisions du 18 octobre 2026

| Question | Décision |
|---|---|
| Syntaxe visée par défaut | VitePress étendu : `::: tip`, ` ```mermaid `, `{#ancre}`, `[[toc]]`, `==surligné==` |
| Propriétés personnalisées de Word | À la racine du front matter : `toto: valeur` |
| Diagrammes Mermaid dans l'éditeur | Rendu dans le document, la source s'ouvre d'un clic avec aperçu en direct |
| Paragraphes et listes dans une cellule de tableau | `<br>` autorisé, seul élément HTML admis, et seulement dans les cellules |

## 11. Avancement

| Lot | État | Détail |
|---|---|---|
| 1 | Livré le 2026-10-18 | Les six pertes de la section 2.2 sont corrigées. Un fichier VitePress complet ressort identique après ouverture et enregistrement, et un second passage ne change rien. Les blocs de code gardent leur langage et restent séparés, l'Entrée prolonge le bloc courant. Conteneurs `:::` et alertes gardent leurs limites. Le HTML, les commentaires, `[[toc]]`, les inclusions, les images introuvables, `~indice~` et les listes de définitions sont gardés tels quels, affichés en source grisée. Un conteneur imbriqué dans un autre est gardé tel quel. |
| 2 | En partie, le 2026-10-18 | Le front matter est gardé tel qu'écrit : seule la ligne d'une propriété modifiée est réécrite, à sa place et dans sa forme (`tags` reste `tags`, une liste reste une liste). Un document Word converti y reçoit date, dernière modification, éditeur, révision, catégorie, société, responsable et ses propriétés personnalisées, à la racine (`toto: "valeur perso"`). Les propriétés techniques (Zotero, étiquettes MSIP, celles qui commencent par `_`) ne sont pas écrites ; un nom qui collisionne avec une clé connue devient `word-<nom>`. Fichier › Informations édite titre, sujet, auteur, mots clés, commentaires, catégorie, et tous les autres champs du front matter, avec ajout et retrait. Commandes `file.frontMatter` et `file.setFrontMatter` pour la CLI et le MCP. Reste : les modèles de front matter (Note, Rapport…). |
| 3 | En partie, le 2026-10-18 | La barre de titre, le titre centré et la question « Enregistrer les modifications ? » montrent le `title` du front matter. La liste des récents montre ce titre, avec le nom du fichier et son chemin en dessous. Reste : les documents de forge dans les récents, le titre demandé au premier enregistrement, la proposition de renommer le fichier. |
| 7 | Livré le 2026-10-19 | Les paragraphes et sauts de ligne d'une cellule sont séparés par `<br>`. Une liste dans une cellule garde ses puces ou ses numéros (`- un<br>- deux`, `1. ouvrir<br>2. enregistrer`). Le gras qui couvre tout un en-tête est retiré. Un tableau imbriqué est écrit après son tableau, avec un renvoi `[↓ 1]` dans la cellule et un libellé `[1] Ligne – Colonne`. `<br>` dans une ligne de tableau n'est plus compté comme du HTML. Le résultat est stable dès le premier enregistrement. |

