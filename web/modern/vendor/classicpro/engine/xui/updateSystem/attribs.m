#ifndef included
#error This script can only be compiled as a #include
#endif

#ifndef __ATTRIBS_M
#define __ATTRIBS_M

#include <lib/config.mi>


// this is the page that maps its items to the options menu, you can add attribs or more pages (submenus)
#define CUSTOM_OPTIONSMENU_ITEMS "{1828D28F-78DD-4647-8532-EBA504B8FC04}"

// custom options submenu item page, you can add more, just use guidgen and Config.newItem()
#define CUSTOM_PAGE "{02CDB0CB-3998-4606-94A7-DF90E7366439}"

Function initAttribs();

Global ConfigAttribute autoupdate_attrib;


initAttribs() {
	ConfigItem custom_page = Config.newItem("Skin Configuration", CUSTOM_PAGE);
	ConfigItem custom_options_page = Config.getItem(CUSTOM_OPTIONSMENU_ITEMS);
	autoupdate_attrib = custom_options_page.newAttribute("Check for ClassicPro Updates at Startup?", "1");
}


#endif