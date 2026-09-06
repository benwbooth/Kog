/*---------------------------------------------------
-----------------------------------------------------
Filename:	loadattribs.m
Version:	1.2

Type:		maki/attrib loader
Date:		29. Aug. 2006 - 23:43 
Author:		Martin Poehlmann aka Deimos
E-Mail:		martin@skinconsortium.com
Internet:	www.skinconsortium.com
		www.martin.deimos.de.vu
-----------------------------------------------------
---------------------------------------------------*/

#define MAIN_ATTRIBS_MGR
#define MAIN_ATTRIBS_LOADER

#include <lib/std.mi>

//#include attribs/init_browser.m
#include ../../scripts/attribs/init_notifier.m
#include attribs/init_Autoresize.m
#include attribs/init_Playlist.m
//#include ../../scripts/attribs/init_songticker.m

Global Configattribute FontRenderer, findOpenRect;
Global String findOpenRect_default;
Global configAttribute skin_attrib;

System.onScriptLoaded(){

	//sorted by alphabet
	//initAttribs_Browser();
	initAttribs_notifier();
	//initAttribs_Songticker();
	initAttribs_Autoresize();
	initAttribs_Playlist();
	

	// Turn 'find open rect' temporary off
	findOpenRect = config.getItemByGuid("{280876CF-48C0-40BC-8E86-73CE6BB462E5}").getAttribute("Find open rect");
	findOpenRect_default = findOpenRect.getData();
	findOpenRect.setData("0");
}

System.onScriptUnloading(){
	findOpenRect.setData(findOpenRect_default);
}

findOpenRect.onDataChanged(){
	if (getData() == "0") return;
	findOpenRect.setData("0");
}
