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

#include attribs/init_browser.m
#include ../../scripts/attribs/init_notifier.m
//#include ../scripts/attribs/init_Autoresize.m

Global Configattribute FontRenderer, findOpenRect;
Global String FontRenderer_default, findOpenRect_default;
Global configAttribute skin_attrib;

System.onScriptLoaded()
{
	//initAttribs_Autoresize();
	initAttribs_notifier();
	initAttribs_Browser();
}