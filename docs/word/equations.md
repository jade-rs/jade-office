# Equations in Word, and in jade

Word has had three ways of putting mathematics in a document. This page
lists every structure of each, the properties Word writes for it, the
linear formats Word accepts, which Word versions brought what, and what
jade does with each item: whether it is **read**, **drawn** as Word draws
it, **edited** from jade's UI, and **saved** back without loss.

Sources: ECMA-376 Part 1 §22.1 (Office Math Markup Language) and §17.16.5.20
(the EQ field), Unicode Technical Note #28 (UnicodeMath), the OpenType MATH
table specification, and Word for Mac's own output (the `math` fixture's
Word for Mac 16.114 PDF in `jade-core/tests/word-oracle`). Version history
is given as far as Microsoft's own release notes state it; entries marked
*(approx.)* are the release generally reported, not a documented build.

## The three generations

| | Format in the file | Word versions | jade |
|---|---|---|---|
| EQ field | `{ EQ … }` field code, result cached as text | Word for Windows 1.x/2.0 onwards; still in every Word | read, drawn from its switches (see [EQ field](#the-eq-field)) |
| Microsoft Equation 3.0 | OLE object `Equation.3` (MathType format) with a WMF/EMF preview | Word 97–2016; removed by the January 2018 security update | the object's preview is drawn; `Equation.3` data is converted to Office Math on load (`word_document.rs`) |
| Office Math (OMML) | `m:oMath` / `m:oMathPara` in the document part | Word 2007 (Windows), Word for Mac 2011, every Word since | read, drawn, edited and saved — the rest of this page |

All OMML structures were defined at once: ECMA-376 1st edition (2006) and
Word 2007 have every structure below. What later versions added is in the
input and conversion layers:

| Feature | Word (Windows) | Word for Mac | jade |
|---|---|---|---|
| OMML structures, professional (2D) display | 2007 | 2011 | drawn (`math_layout::styled`) |
| Linear format, UnicodeMath, auto build-up | 2007 | 2011 | typed and built up (`math_layout::linear::unicode`) |
| Math AutoCorrect (`\alpha` → α) | 2007 | 2011 | applied as linear text is read (`linear::autocorrect`) |
| Professional ⇄ linear conversion | 2007 | 2011 | 2 次元形式 / 行形式 (`EquationEditor::to_professional_form`, `to_linear_form`) |
| Built-in equation gallery | 2007 | 2011 | 挿入 > 数式 ▾ (`BUILT_IN_EQUATIONS`) |
| Equation Tools > Design tab (now 数式 / Equation) | 2007 | 2011 (Document Elements), 2016 tab | 数式 tab while an equation is edited (`word_equation.rs`) |
| Ink Equation (手書き数式) | 2016 | — | not offered |
| LaTeX linear format | Microsoft 365, 2017–2018 *(approx.)* | Microsoft 365 *(approx.)* | typed, built up and written (`linear::latex`) |
| `#` equation numbers in UnicodeMath | Microsoft 365 *(approx.)* | Microsoft 365 *(approx.)* | not read |

## Where an equation sits

| Element | Meaning | Properties | jade |
|---|---|---|---|
| `m:oMath` | an equation; inline when it shares its paragraph with text | — | read; drawn as an object in its line, as tall and deep as its structure; edited in place; saved as written |
| `m:oMathPara` | display math: one or more `m:oMath` on lines of their own, alone in a `w:p` | `m:oMathParaPr/m:jc`: `left`, `right`, `center`, `centerGroup` (default) | read; each `m:oMath` on its own lines, justified as a group (`centerGroup`) or line by line (`center`); edited and saved |
| `m:mathPr` (settings part) | document-wide defaults | `mathFont`, `brkBin` (before/after/repeat), `brkBinSub`, `smallFrac`, `dispDef`, `lMargin`, `rMargin`, `defJc`, `preSp`, `postSp`, `interSp`, `intraSp`, `wrapIndent` (twips, default 1440) / `wrapRight`, `intLim` (subSup default), `naryLim` (undOvr default) | read (`settings::MathProperties`); `mathFont` applies to inline, display and text-box equations; `intLim`, `naryLim`, `smallFrac`, `wrapIndent`/`wrapRight`, `brkBin` (before, after), margins and `defJc` used by the layout; `dispDef=false` disables math margins and continuation settings and uses the paragraph alignment cascade; explicit equation alignment still wins; the four equation-spacing settings are preserved without affecting layout, consistent with the Word implementation notes below; saved as written |

### Equation spacing compatibility

The published Word implementation notes say Word ignores `preSp`, `postSp`,
`interSp`, and `intraSp` (and does not write them back). jade preserves these
settings on save but also leaves them out of layout. Their presence in the
OOXML schema alone does not establish a missing Word layout feature. New Word
captures are still needed to check whether current Word releases differ from
these implementation notes.

Sources: [preSp](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/e03f7edc-179c-4017-8869-2a9930c37c68),
[postSp](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/2ec60dcb-1daf-4437-9ec0-729017b3dc21),
[interSp](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/2294400c-b0fb-499d-aa04-c38fc4028c88),
[intraSp](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/0c50cee0-a9b4-4139-8e36-b86a8a290208).

### Literal run compatibility

The OOXML `m:lit` definition suppresses operator interpretation, but the
published Word implementation note says Word 2007, SP1 and SP2 do not change
operator interpretation for this property. Current Word captures are needed
before treating literal-run spacing as a confirmed compatibility gap; jade
currently preserves the property without applying it to layout.

Source: [Word implementation note for lit](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/e941fac0-a989-49a8-a8f0-766a141b73a1).

## Structures

Every structure's properties element (`…Pr`) may end with `m:ctrlPr`, the
`w:rPr` (and `w:ins`/`w:del`) of the structure's own characters. jade keeps
it on save and reads its formatting where the structure draws characters.

| Element | Word's name (ja / en) | Properties (defaults) | Arguments | jade layout |
|---|---|---|---|---|
| `m:f` | 分数 / Fraction | `type`: `bar` (default), `skw` skewed, `lin` linear, `noBar` stacked | `num`, `den` | bar: numerator and denominator clear the rule by the MATH gaps, rule on the math axis, display size in display math and script size inline (`smallFrac` keeps it small); `skw` a slash between raised and lowered parts; `lin` `a/b` on the line; `noBar` stack gaps |
| `m:rad` | べき乗根 / Radical | `degHide` (off) | `deg`, `e` | the face's √ variant or assembly grown to the base (a drawn sign without a face), vinculum over the base, degree at script-script size raised by `RadicalDegreeBottomRaisePercent` over the sign's arm |
| `m:sSup`, `m:sSub`, `m:sSubSup` | 上付き/下付き文字 / Script | `sSubSupPr/alnScr` (off) | `e`, `sub`, `sup` | MATH shifts (`SuperscriptShiftUp`, cramped variant, baseline drops, `SubSuperscriptGapMin`), italic correction before a superscript, `SpaceAfterScript`; scripts at 70% of the run as Word sets them |
| `m:sPre` | 左上付き/左下付き文字 / Pre-script | — | `sub`, `sup`, `e` | scripts right-aligned before the base, same shifts |
| `m:nary` | 積分, 大型演算子 / Integral, Large Operator | `chr` (∫), `limLoc` (`intLim`/`naryLim`), `grow` (off), `subHide`, `supHide` | `sub`, `sup`, `e` | display math takes the face's display variant (integrals about two ems, as Word draws them), centred on the axis; limits over/under (`undOvr`) or beside (`subSup`, italic correction moves the lower one in); inline math puts limits beside; `grow` stretches the operator to its operand |
| `m:d` | かっこ / Bracket (delimiter) | `begChr` `(`, `sepChr` `|`, `endChr` `)`, `grow` (on), `shp` (`centered` default, `match`) | one `e` per item | delimiters grow to enclose their content symmetrically about the axis (`centered`) or exactly (`match`), with TeX's delimiter factor and shortfall; the face's variants and assemblies, or drawn paths; empty `begChr`/`endChr` draw nothing; separators between items |
| `m:func` | 関数 / Function | — | `fName`, `e` | name upright (`m:sty p`), a thin space before the argument unless it opens with a bracket |
| `m:limLow`, `m:limUpp` | 極限と対数 / Limit | — | `e`, `lim` | limit at script size centred under or over the base, MATH limit gaps |
| `m:acc` | アクセント / Accent | `chr` (U+0302 ̂) | `e` | over one letter: the face's combining mark centred on the letter's top-accent attachment, raised over tall bases past `AccentBaseHeight`; over a wider base: stretched (hat, tilde, arrows, bar, breve, check, dots drawn as wide marks) |
| `m:bar` | 上線/下線 / Bar | `pos` (`bot` default, `top`) | `e` | MATH overbar/underbar gap, rule and extra ascender/descender |
| `m:groupChr` | 演算子 (上下のかっこ) / Group Character | `chr` (U+23DF ⏟), `pos` (`bot`), `vertJc` (`bot`) | `e` | the brace (or bracket, parenthesis, arrow) stretched to the base's width, under or over it; `vertJc` moves the baseline to the character when it says so |
| `m:borderBox` | 囲み / Border Box | `hideTop`, `hideBot`, `hideLeft`, `hideRight`, `strikeH`, `strikeV`, `strikeBLTR`, `strikeTLBR` (all off) | `e` | the four sides less the hidden ones, the strikes through the box, padding round the base |
| `m:box` | ボックス / Box | `opEmu`, `noBreak`, `diff`, `brk`, `aln` (off) | `e` | groups its base; `opEmu` spaces it as the operator it holds, `diff` puts a thin space before a differential |
| `m:phant` | ファントム / Phantom | `show` (on), `zeroWid`, `zeroAsc`, `zeroDesc`, `transp` (off) | `e` | hidden content keeps its room; the zero switches take its width, ascent or depth away |
| `m:eqArr` | 数式配列 / Equation Array | `baseJc` (`center`), `maxDist`, `objDist`, `rSpRule` (0 single, 1 1.5×, 2 double, 3 exact `rSp`, 4 multiple), `rSp` | one `e` per row | rows aligned on their `&` marks (odd marks right-align the column before them, even marks left-align the one after, as `&` pairs do in Word), rows spaced by `rSpRule`, centred on the axis or aligned on the top or bottom row; `maxDist`/`objDist` are read and not applied |
| `m:m` | 行列 / Matrix | `baseJc` (`center`), `plcHide`, `rSpRule`, `rSp`, `cGpRule`, `cGp`, `cSp`, `mcs/mc/mcPr` (`count`, `mcJc`: `center`, `left`, `right`) | `mr` rows of `e` cells | column widths from the widest cell, each column justified by `mcJc`, column gap by `cGpRule`/`cGp` (one em by default), minimum spacing `cSp`, rows like an equation array; empty cells keep a placeholder's room unless `plcHide` |
| `m:r` | — | `m:rPr`: `lit`, `nor` (normal text), `scr` (`roman`, `script`, `fraktur`, `double-struck`, `sans-serif`, `monospace`), `sty` (`p`, `b`, `i` default, `bi`), `brk` (`alnAt`), `aln`; and the run's `w:rPr` | `m:t` (and Word run content) | letters from the Mathematical Alphanumeric Symbols in the run's style and alphabet (Word's italic default: Latin letters and lowercase Greek; capital Greek upright), hyphen shown as minus, atoms spaced by class as TeX spaces them, italic correction before an upright glyph; `nor` text in the document's text face with no operator spacing; `brk` and `aln` break and align display lines |

### Line breaking and alignment

Display equations break where a run carries `m:brk` (aligned under the
`alnAt`-th operator of the first line, else under its first relation) and,
past the column's width, before a binary or relation operator at the top
level (`brkBin`), continuation lines indented by `wrapIndent` or set flush
right with `wrapRight`. Runs with `m:aln` align their lines on that point.
Inline equations do not break.

Text-box paragraphs keep inline equations as measured objects in the same
wrap engine as their text. The renderer paints their equation plans,
reserves ascent and descent, and registers caret fragments and equation hit
zones. This includes manual line breaks, narrow boxes, and left, centre or
right alignment for paragraphs containing only an equation. The
`text_box_inline_math_wraps_paints_and_registers_object_positions` and
`text_box_math_only_paragraph_keeps_alignment_on_repaint` regressions cover
these paths; fresh Word capture comparisons remain pending.
Inline equations can also be inserted at a text-box caret. The
`inline_equation_in_a_text_box_preserves_both_copies_and_history` regression
covers insertion after Japanese text, both DrawingML/VML copies, save/reopen,
paragraph alignment, neighbouring body text and undo/redo. Inline insertion
leaves the caret after the equation's UTF-8 object placeholder in both body
and text-box paragraphs.
Inline equations in text-box paragraphs also use the equation editor's
existing replacement command. The
`a_text_box_inline_equation_saves_reopens_undoes_and_redoes_in_its_story`
regression checks that editing the target equation preserves the body and
neighbouring equations, text, paragraph alignment and shape attributes
through save/reopen and undo/redo.

Display equations in text boxes use the same editor through their own
story locations, including equations inside a box's table cells. The
`a_text_box_display_equation_in_a_table_keeps_its_story_on_save_and_undo`
regression checks save/reopen and undo/redo while preserving body equations,
other box equations, paragraph alignment and shape attributes.
`a_text_box_display_equation_updates_drawingml_and_vml_fallback_together`
checks that both representations of an imported box receive the edit.
Inline edits also update both DrawingML and VML fallback copies. The
`a_text_box_inline_equation_updates_drawingml_and_vml_fallback_together`
regression edits the second equation in a box paragraph and checks both
copies, neighbouring equations, save/reopen and exact undo/redo XML.
Equation editing in notes remains unsupported.

The equation toolbar recognizes an inline equation at either boundary of
its rendered object, including immediately after its three-byte U+FFFC
placeholder. Regressions cover Japanese prefixes and adjacent equations;
carets in neighbouring text or other paragraphs do not select that object.

Equation-array distribution remains unimplemented. The published Word
implementation note for [`maxDist`](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/4a8c035c-c930-4e87-aabf-32715949ba48)
limits its effect to arrays with alignment points. Future Word comparisons
must include arrays with and without these points, rather than assuming
that every array expands when the setting is enabled.

### Faces and metrics

The document's `mathFont` setting selects the math face, with Cambria Math
as the fallback. Normal-text (`m:nor`) runs without an explicit run font
resolve the containing paragraph style, its base styles, document text
defaults, and theme font. Explicit run fonts remain authoritative. The
`normal_math_text_inherits_the_equation_paragraph_style` regression covers
this cascade for display, inline, and text-box equation plans. Fresh Word
captures for these font and display-default changes remain pending; renderer
regression tests establish implementation behavior, not Word equivalence.

By default, Word sets equations in Cambria Math and lays them out from its OpenType
MATH table. The native shell discovers `m:mathFont` in document settings
and binds the configured family's MATH table and glyph outlines, keeping
Cambria Math as the default fallback (face 1 of Office's `Cambria.ttc`).
The selected faces survive page-geometry rebuilds, PDF output and DOCX
reopen; `a_custom_math_face_survives_page_rebuild_and_pdf_export` covers
this with a bundled synthetic MATH font. With no face bound (the browser
build, or a machine without Office) the same placement rules run on Cambria
Math's published constants with the painter's glyphs, and stretched
characters are drawn as paths. Calibrated against Word for Mac's `math`
oracle: scripts at 70% of the run (not the table's 73%), n-ary operators
centred on the math axis, display integrals the face's two-em variant, and
italic correction after an italic letter before an upright glyph.

## Linear formats

Both formats read into the same structure tree and are written as OMML;
any OMML is written back in either (`math_layout::linear`). Math
AutoCorrect runs first, so `\alpha`, `\sum` and the rest work in both.

### UnicodeMath (Unicode Technical Note #28)

```
expression   := (element | space)*
element      := operand postfix* | structure | "\"" text "\"" | "\" char
operand      := letters-and-digits | "(" expression ")" | "〖" expression "〗"
postfix      := ("^" | "_") argument          scripts (one of each)
              | combining-accent              x̂ → m:acc
              | "┬" argument | "┴" argument   limit below / above
argument     := operand | sign operand | structure
fraction     := operand ("/" | "∕" | "⁄" | "¦" | "⊘") operand
                                              bar, linear, skewed, stacked
structure    := "√" argument | "√(" expr "&" expr ")" | "∛" argument | "∜" argument
              | nary ("_" argument)? ("^" argument)? ("▒" operand | operand)?
              | open items close              delimiters; items split by "│" or "∣"
              | "├" open? items "┤" close?    a side drawn as nothing
              | "■(" cells ")"                matrix: "&" between cells, "@" between rows
              | "█(" rows ")"                 equation array: "@" between rows, "&" aligns
              | "⒨(" … ")" | "⒩(" … ")"      matrix in ( ) or ‖ ‖
              | "Ⓒ(" rows ")"                 cases: { and an equation array
              | "□" argument | "▭" argument   box, border box
              | "¯" argument | "▁" argument   overbar, underbar
              | "⟡" argument                  phantom
              | ("⏞"|"⏜"|"⎴") argument ("^" argument)?   over-group (+ label)
              | ("⏟"|"⏝"|"⎵") argument ("_" argument)?   under-group (+ label)
              | "_" a "^" b " " operand       prescripts
nary         := ∑ ∏ ∐ ∫ ∬ ∭ ⨌ ∮ ∯ ∰ ∱ ∲ ∳ ⋀ ⋁ ⋂ ⋃ ⨀ ⨁ ⨂ ⨄ ⨆
```

Parentheses around a fraction's or a script's operand group it and are
dropped (`(a+b)/c`); elsewhere they are delimiters that grow. A function
name (`sin`, `cos`, `tan`, `log`, `ln`, `lim`, `max`, `min`, `exp`, `det`,
the hyperbolic and inverse ones and the rest of Word's list) applies to the
operand after it, explicitly with U+2061 FUNCTION APPLICATION; `lim_(…)`
puts the limit under the name. Writing OMML out uses the same notation:
`x=(−b±√(b^2−4ac))/2a` reads back to the same tree.

Typing in jade builds up as Word does: the argument the caret is in is
read again when a space follows linear format or a control word.

### LaTeX

The common math subset Word accepts: `\frac`, `\dfrac`, `\tfrac`, `\cfrac`,
`\binom`, `\sqrt[n]{}`, `^{}` and `_{}`, the n-ary operators (`\sum`,
`\prod`, `\int`, `\oint` … with `\limits` / `\nolimits`), `\left` …
`\middle` … `\right` with `.` for an empty side, the environments `matrix`,
`pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix`, `Vmatrix`, `cases`, `aligned`,
`align`, `gathered`, `split` and `array`, accents (`\hat`, `\widehat`,
`\tilde`, `\bar`, `\vec`, `\dot`, `\ddot`, `\dddot`, `\check`, `\breve`,
`\acute`, `\grave`, `\mathring`, `\overleftarrow`, `\overrightarrow`),
`\overline`, `\underline`, `\overbrace{}^{}`, `\underbrace{}_{}`,
`\overset`, `\underset`, `\boxed`, `\phantom`, `\text`, `\mathrm`,
`\operatorname`, `\mathbf`, `\mathit`, `\boldsymbol`, `\mathbb`,
`\mathcal`, `\mathscr`, `\mathfrak`, `\mathsf`, `\mathtt`, the function
names, spacing commands and every Math AutoCorrect control word as a
symbol. Written out, OMML becomes the same commands (`\frac{a}{b}`, a
matrix in brackets as `pmatrix` …).

### Math AutoCorrect

Word's built-in list (File > Options > Proofing > AutoCorrect Options >
Math AutoCorrect): about 300 control words — Greek letters, operators,
relations, arrows, the structure characters (`\matrix` ■, `\eqarray` █,
`\naryand` ▒, `\sqrt` √ …), accents and spaces — and the operator
shortcuts `!=`, `<=`, `>=`, `+-`, `-+`, `->`, `<-`, `<->`, `::`, `:=`,
`~=`, `<<`, `>>`. The list is `MATH_AUTOCORRECT` in
`math_layout/linear/autocorrect.rs`; the space that ends a control word is
consumed with it, as in Word. Word's built-in equation entries
(`\quadratic`, `\binomial`, `\integral`, `\limit`) expand to their
UnicodeMath. User-defined entries and "use Math AutoCorrect rules outside
of math regions" are not supported.

## The EQ field

`EQ` switches (§17.16.5.20) are read into the same structures
(`math_layout::eq_field::eq_field_elements`, for the field engine as well
as the renderer) and laid out in the field's own face and size, upright,
at one size throughout, as Word draws an EQ field:

| Switch | Word's meaning | jade |
|---|---|---|
| `\f(a,b)` | fraction | `m:f` |
| `\r(n,x)`, `\r(x)` | radical, with an index | `m:rad` |
| `\s\up n(a)`, `\s\do n(a)`, `\s\ai n`, `\s\di n` | raise or lower by n points; stack arguments with extra space above or below | `eqShift` |
| `\b\lc\c \rc\c \bc\c(a)` | brackets sized to the content: left, right, or both (matching pair) | `m:d`, growing |
| `\a\al \ac \ar \co n \vs n \hs n(…)` | array: alignment, columns, vertical and horizontal spacing in points | `eqArray` |
| `\l(a,b,…)` | list, comma separated | `eqList` |
| `\o\al \ac \ar(a,b,…)` | overstrike, each argument over the others | `eqOverstrike` |
| `\x\to \bo \le \ri(a)` | box, or only the named sides | `m:borderBox` with hidden sides |
| `\i\su \pr \in \fc\c \vc\c(lower,upper,integrand)` | integral; summation; product; inline form; fixed or variable character | `m:nary` |
| `\d\fo n \ba n \li()` | displace the next character forward or back, underlining the gap | `eqDisplace` |

Escaped characters (`\,` `\(` `\)` `\\`) are literal. `EQ \* jc` ruby,
enclosed characters and distributed text keep their own handling.

## Editing in jade

- 挿入 > 数式 puts a new equation at the caret: display in an empty
  paragraph, inline in text, as Word chooses. Its list inserts one of Word's
  nine built-in equations (2 次方程式の解の公式, 2 項定理, 円の面積, フーリエ級数,
  ピタゴラスの定理, 和の展開, テイラー展開, 三角関数の公式 1 and 2).
- Clicking an equation edits it in place, framed, with the 数式 tab:
  ツール (the built-in list), 変換 (Unicode / LaTeX, 2 次元形式, 行形式,
  標準テキスト), 記号と特殊文字 (基本数式, ギリシャ文字, 文字形の記号, 演算子,
  矢印, 否定の関係演算子, 幾何学) and 構造 (分数, 上付き/下付き文字, べき乗根,
  積分, 大型演算子, かっこ, 関数, アクセント, 極限と対数, 演算子, 行列), each
  inserting a template with empty arguments at the caret.
- The caret moves inside the structure: ← → into and out of arguments
  (numerator, then denominator, then past the fraction), ↑ ↓ between
  arguments above and below (numerator and denominator, superscript and
  subscript, matrix rows), Tab to the next argument, Home and End.
  Backspace deletes a character, selects a structure and deletes it on the
  second press, and takes an empty structure away from inside it. Escape
  or Enter leaves the equation.
- In a matrix or an equation array the tab adds 行列: rows inserted above or
  below, columns before or after, a row or column deleted, the column
  justified left, centred or right (`m:mcs`). 配置 inserts a manual line
  break before the caret (⇧Enter too) or an alignment point (`m:brk`,
  `m:aln`).
- ホーム's font size, colour and bold apply to the equation being edited
  (`w:sz`, `w:color`, `m:sty`), as Word applies them to a selected
  equation.
- Typed text goes into the argument at the caret; a space after linear
  format builds it up. Every edit rewrites the equation's OMML in place
  (`ReplaceEquation`), one undo step each.

## What is not done

- Ink Equation, user Math AutoCorrect entries, and Math AutoCorrect outside
  equations.
- `brkBin` `repeat` (breaks as `before`), `brkBinSub`,
  `eqArr` `maxDist`/`objDist`.
- Current Word behavior for `m:lit` (see the implementation-note caveat above).
- Breaking inline equations across lines.
- UnicodeMath equation numbers (`#`) and the `\eqno` form.
- Selecting a range inside an equation (the caret is a point; a structure
  can be selected for deletion).
