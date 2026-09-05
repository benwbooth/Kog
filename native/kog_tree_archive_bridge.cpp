#include "kog_tree_archive_bridge.h"
#include "kog_tree_archive.h"
#include <QtCore/QByteArray>

void kogConfigureArchiveDecoder(rust::Fn<rust::String(rust::Slice<const uint8_t>)> decoder)
{
    kogSetArchiveNameDecoder([decoder](const QByteArray &bytes) {
        const auto text = decoder({reinterpret_cast<const uint8_t *>(bytes.constData()), size_t(bytes.size())});
        return QString::fromUtf8(text.data(), qsizetype(text.size()));
    });
}
