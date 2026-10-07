# 2. The MML view

## Opening it

The MML view is part of the Channel Inspector.

| Player | How to open it |
| --- | --- |
| Desktop (Qt) | **View → Channel Inspector** (or **Ctrl+Shift+I**), then choose **MML score** in the view menu |
| Web player | Open the Channel Inspector, then choose **MML score** |
| Terminal | Open the Channel Inspector from the View menu, then press **4** |
| Android and iOS | Open **Channel Inspector** from Now Playing, then choose **MML** |

The **Guide** button next to the score opens this book.

## Recording

When you open the view for a song, Kog starts recording its score in the
background. Recording runs faster than playback: from a few times real time
for PlayStation music to hundreds of times for NES music. The first bars appear
after a few seconds of audio have been recorded, and the score grows while
recording continues. A status line shows how far recording has got, for
example `Still recording… 1:12 of 2:38`.

Songs without a known length (some chip music loops forever) are recorded for
ten minutes.

## Following playback

The block of bars that is playing is outlined, and every note that is sounding
is highlighted. A note that runs across a bar line is highlighted in each
piece, so held chords stay lit until they end.

With **Follow playback** on, the view scrolls so the playing block stays in
sight. Scroll away and turn following off to read ahead or back; the
highlighting continues wherever you are.

## Bars per line

By default each line holds four bars of one track. Change it with:

| Player | Control |
| --- | --- |
| Desktop | the **Bars per line** box in the toolbar |
| Web player | the **Bars per line** selector (remembered in your browser) |
| Terminal | **+** and **-** |
| Android and iOS | the **Bars per line** stepper |

Changing it rearranges the recorded score; it does not record the song again.

## Copying and saving

**Copy** puts the whole score on the clipboard, and **Export** saves it as a
`.mml` file named after the song's title. Both take the full text, headers
included, laid out with the current bars per line; a score that is still
recording is copied as far as it has got.

| Player | Copy | Export |
| --- | --- | --- |
| Desktop | **Copy** | **Export…**, then choose where to save |
| Web player | **Copy** | **Export** downloads the file |
| Terminal | **Y** (through the terminal's clipboard, OSC 52) | **E** saves into the folder Kog was started from |
| Android and iOS | **Copy** | **Export**, then choose where to save |

The file can be read back exactly: see chapter 17.

## Colours

Tokens are coloured by what they are:

| Colour | Tokens |
| --- | --- |
| White, bold | note names (`c`, `f+`) |
| Light blue | lengths and ties (`8.`, `^16`) |
| Purple | octave changes (`o4`, `<`, `>`) and macros (`~3`) |
| Grey | rests (`r`) and comments |
| Orange | unpitched hits (`x`) and detune (`(+12)`) |
| Blue and green | parameter names and values (`Period=253`) |
| Gold | track labels and instruments (`@"Pulse duty 1"`) |
| Cyan | velocity, level, pan and bend (`V100`, `v750`) |
| Red | legato marks (`&`) |
