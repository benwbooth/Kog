// Registers `import GlobalMenu` (GlobalMenuBar) for the QML; called from
// main.rs before the engine loads Main.qml.
#include <QtQml/qqml.h>

#ifdef KOG_GLOBAL_MENU
#include "global_menu.h"
using KogGlobalMenuBar = GlobalMenuBar;
#else
#include "global_menu_fallback.h"
using KogGlobalMenuBar = GlobalMenuBarFallback;
#endif

extern "C" void kog_register_global_menu()
{
    qmlRegisterType<KogGlobalMenuBar>("GlobalMenu", 1, 0, "GlobalMenuBar");
}
