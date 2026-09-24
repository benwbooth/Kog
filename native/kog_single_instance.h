#pragma once

#include <QtCore/QString>

// Returns "primary", "raised", or a diagnostic explaining why no GUI can start.
QString kogSingleInstanceStart();
