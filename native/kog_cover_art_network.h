#pragma once

#include <QtCore/QByteArray>
#include <QtCore/QString>

// Blocking cover-art download for the album-art worker. Never call on the
// GUI thread. Only the cover provider hosts below are allowed, and
// redirects must stay inside them.
QByteArray kogFetchCoverArtUrl(const QString &address, unsigned int maxBytes);
