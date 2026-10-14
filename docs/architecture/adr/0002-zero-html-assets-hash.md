# ADR 0002 — Zéro HTML, commentaires HTML seuls tolérés, ressources nommées par contenu

- Statut : accepté pour l'incrément 1 (14/10/2026)
- Contexte : exigence « zéro HTML hormis pour les commentaires » ; documents versionnés dans Git et lus sur GitHub/GitLab/Forgejo/VitePress.
- Décision : le fichier ne contient que du CommonMark + GFM ; seules les balises de commentaire `<!-- -->` sont permises (vérifié par `markdown::html_lines`). Les images sont écrites dans `assets/img-<FNV-64 du contenu>.<ext>`.
- Conséquences : `<br>` dans les cellules et `<a id>` des signets disparaissent (signalés) ; les tailles d'images ne sont pas conservées en GFM pur ; les diffs Git restent stables (un réenregistrement ne renomme aucune image) et les images identiques sont dédoublonnées.
