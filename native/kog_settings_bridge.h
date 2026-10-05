#pragma once
#include "kog_settings.h"
#include "rust/cxx.h"
inline void kogConfigureSettings(rust::Fn<QString(const QString &, const QString &, bool)> callback)
{
    kogSetSettingsPort([callback](const QString &key, const QString &value, bool write) {
        return callback(key, value, write);
    });
}
