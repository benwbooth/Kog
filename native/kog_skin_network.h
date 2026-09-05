#pragma once
#include <QtCore/QByteArray>
#include <QtCore/QString>
QByteArray kogFetchSkinUrl(const QString &url, unsigned int maxBytes);
bool kogValidateSkinImage(const QString &path, unsigned int minWidth, unsigned int minHeight);
QString kogSkinTextColors(const QString &path);
