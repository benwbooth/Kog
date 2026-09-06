/*---------------------------------------------------
-----------------------------------------------------
Filename:	visualizer.m
Version:	1.4

Type:		maki
Date:		07. Okt. 2007 - 19:56  
Author:		Martin Poehlmann aka Deimos
E-Mail:		martin@skinconsortium.com
Internet:	www.skinconsortium.com
		www.martin.deimos.de.vu

Note:		Edited by pjn123 for ClassicPro
-----------------------------------------------------
---------------------------------------------------*/

#include <lib/std.mi>

Function refreshVisSettings();
Function setVis (int mode);
Function ProcessMenuResult (int a);

Global Group scriptGroup;
Global Vis visualizer, visualizer_m;
Global Text tmr;

Global PopUpMenu visMenu;
Global PopUpMenu specmenu;
Global PopUpMenu oscmenu;
Global PopUpMenu pksmenu;
Global PopUpMenu anamenu;
Global PopUpMenu stylemenu;

Global Int currentMode, a_falloffspeed, p_falloffspeed, a_coloring;
Global Boolean show_peaks, isShade, doubleClick;
Global layer trigger, overlay1, overlay2;

/*Global Layout thislayout;
Global Container main;*/

System.onScriptLoaded()
{ 
	scriptGroup = getScriptGroup();
	/*thislayout = scriptGroup.getParentLayout();
	main = thislayout.getContainer();*/

	// this script runs in shade and normal layout, we store this bool to prevent some actions to recieve the shade vis.
	//isShade = (scriptGroup.getParentlayout().getID() == "shade");

	visualizer = scriptGroup.getObject(getToken(getParam(),";",0));
	overlay1 = scriptGroup.getObject(getToken(getParam(),";",1));
	trigger = scriptGroup.getObject(getToken(getParam(),";",2));

	visualizer.setXmlParam("peaks", integerToString(show_peaks));
	visualizer.setXmlParam("peakfalloff", integerToString(p_falloffspeed));
	visualizer.setXmlParam("falloff", integerToString(a_falloffspeed));
	
	doubleClick = stringToInteger(getToken(getParam(),";",4));

///////////////
	Map myMap = new Map;
	myMap.loadMap(getToken(getParam(),";",3));
	overlay1.setRegionFromMap(myMap, 255, 0);
	//overlay2.setRegionFromMap(myMap, 255, 0);
	delete myMap;
///////////////

	refreshVisSettings ();
}

scriptGroup.onSetVisible(boolean onOff){
	if(onOff){
		refreshVisSettings ();
	}
}
scriptGroup.onResize(int x, int y, int w, int h){
	if(scriptGroup.isVisible()){
		refreshVisSettings ();
	}
}

refreshVisSettings ()
{
	currentMode = getPublicInt("cpro2.Visualizer Mode", 1);
	show_peaks = getPublicInt("cpro2.Visualizer show Peaks", 1);
	a_falloffspeed = getPublicInt("cpro2.Visualizer analyzer falloff", 3);
	p_falloffspeed = getPublicInt("cpro2.Visualizer peaks falloff", 2);
	a_coloring = getPublicInt("cpro2.Visualizer analyzer coloring", 0);

	visualizer.setXmlParam("peaks", integerToString(show_peaks));
	visualizer.setXmlParam("peakfalloff", integerToString(p_falloffspeed));
	visualizer.setXmlParam("falloff", integerToString(a_falloffspeed));

	if (a_coloring == 0)
	{
		visualizer.setXmlParam("coloring", "Normal");
	}
	else if (a_coloring == 1)
	{
		visualizer.setXmlParam("coloring", "Normal");
	}
	else if (a_coloring == 2)
	{
		visualizer.setXmlParam("coloring", "Fire");
	}
	else if (a_coloring == 3)
	{
		visualizer.setXmlParam("coloring", "Line");
	}

	setVis (currentMode);
}

trigger.onLeftButtonDown (int x, int y)
{
	if(doubleClick) return;
	if (isKeyDown(VK_ALT) && isKeyDown(VK_SHIFT) && isKeyDown(VK_CONTROL))
	{
		if (visualizer.getXmlParam("fliph") == "1")
		{
			visualizer.setXmlParam("fliph", "0");
		}
		else
		{
			visualizer.setXmlParam("fliph", "1");
		}
		return;
	}

	currentMode++;

	if (currentMode == 7)
	{
		currentMode = 0;
	}

	setVis	(currentMode);
	complete;
}

trigger.onLeftButtonDblClk (int x, int y)
{
	if(!doubleClick) return;
	if (isKeyDown(VK_ALT) && isKeyDown(VK_SHIFT) && isKeyDown(VK_CONTROL))
	{
		if (visualizer.getXmlParam("fliph") == "1")
		{
			visualizer.setXmlParam("fliph", "0");
		}
		else
		{
			visualizer.setXmlParam("fliph", "1");
		}
		return;
	}

	currentMode++;

	if (currentMode == 7)
	{
		currentMode = 0;
	}

	setVis	(currentMode);
	complete;
}


trigger.onRightButtonUp (int x, int y)
{
	visMenu = new PopUpMenu;
	specmenu = new PopUpMenu;
	oscmenu = new PopUpMenu;
	pksmenu = new PopUpMenu;
	anamenu = new PopUpMenu;
	stylemenu = new PopUpMenu;

	visMenu.addCommand("Presets:", 999, 0, 1);
	visMenu.addCommand("No Visualization", 100, currentMode == 0, 0);
	specmenu.addCommand("Thick Bands", 1, currentMode == 1, 0);
	specmenu.addCommand("Thin Bands", 2, currentMode == 2, 0);
	specmenu.addCommand("Solid Bands", 3, currentMode == 3, 0);
	visMenu.addSubMenu(specmenu, "Spectrum Analyzer");

	oscmenu.addCommand("Solid", 4, currentMode == 4, 0);
	oscmenu.addCommand("Dots", 5, currentMode == 5, 0);
	oscmenu.addCommand("Lines", 6, currentMode == 6, 0);
	visMenu.addSubMenu(oscmenu, "Oscilloscope");

	visMenu.addSeparator();
	visMenu.addCommand("Options:", 102, 0, 1);
	visMenu.addCommand("Show Peaks", 101, show_peaks == 1, 0);
	pksmenu.addCommand("Slower", 200, p_falloffspeed == 0, 0);
	pksmenu.addCommand("Slow", 201, p_falloffspeed == 1, 0);
	pksmenu.addCommand("Moderate", 202, p_falloffspeed == 2, 0);
	pksmenu.addCommand("Fast", 203, p_falloffspeed == 3, 0);
	pksmenu.addCommand("Faster", 204, p_falloffspeed == 4, 0);
	visMenu.addSubMenu(pksmenu, "Peak Falloff Speed");
	anamenu.addCommand("Slower", 300, a_falloffspeed == 0, 0);
	anamenu.addCommand("Slow", 301, a_falloffspeed == 1, 0);
	anamenu.addCommand("Moderate", 302, a_falloffspeed == 2, 0);
	anamenu.addCommand("Fast", 303, a_falloffspeed == 3, 0);
	anamenu.addCommand("Faster", 304, a_falloffspeed == 4, 0);
	visMenu.addSubMenu(anamenu, "Analyzer Falloff Speed");
	stylemenu.addCommand("Normal", 400, a_coloring == 0, 0);
	stylemenu.addCommand("Fire", 402, a_coloring == 2, 0);
	stylemenu.addCommand("Line", 403, a_coloring == 3, 0);
	visMenu.addSubMenu(stylemenu, "Analyzer Coloring");

	ProcessMenuResult (visMenu.popAtMouse());

	delete visMenu;
	delete specmenu;
	delete oscmenu;
	delete pksmenu;
	delete anamenu;
	delete stylemenu;

	complete;	
}

ProcessMenuResult (int a)
{
	if (a < 1) return;

	if (a > 0 && a <= 7 || a == 100)
	{
		if (a == 100) a = 0;
		setVis(a);
	}

	else if (a == 101)
	{
		show_peaks = (show_peaks - 1) * (-1);
		visualizer.setXmlParam("peaks", integerToString(show_peaks));
		setPublicInt("cpro2.Visualizer show Peaks", show_peaks);
	}

	else if (a >= 200 && a <= 204)
	{
		p_falloffspeed = a - 200;
		visualizer.setXmlParam("peakfalloff", integerToString(p_falloffspeed));
		setPublicInt("cpro2.Visualizer peaks falloff", p_falloffspeed);
	}

	else if (a >= 300 && a <= 304)
	{
		a_falloffspeed = a - 300;
		visualizer.setXmlParam("falloff", integerToString(a_falloffspeed));
		setPublicInt("cpro2.Visualizer analyzer falloff", a_falloffspeed);
	}

	else if (a >= 400 && a <= 403)
	{
		a_coloring = a - 400;
		if (a_coloring == 0)
		{
			visualizer.setXmlParam("coloring", "Normal");
		}
		else if (a_coloring == 1)
		{
			visualizer.setXmlParam("coloring", "Normal");
		}
		else if (a_coloring == 2)
		{
			visualizer.setXmlParam("coloring", "Fire");
		}
		else if (a_coloring == 3)
		{
			visualizer.setXmlParam("coloring", "Line");
		}
		setPublicInt("cpro2.Visualizer analyzer coloring", a_coloring);
	}
}

setVis (int mode)
{
	setPublicInt("cpro2.Visualizer Mode", mode);
	overlay1.hide();
	//overlay2.hide();
	if (mode == 0)
	{
		visualizer.setMode(0);
	}
	else if (mode == 1)
	{
		if(overlay1.getWidth()<71)
		{
			overlay1.show();
			visualizer.setXmlParam("bandwidth", "thin");
			visualizer.setMode(1);
		}
		else
		{
			visualizer.setXmlParam("bandwidth", "wide");
			visualizer.setMode(1);
		}
	}
	else if (mode == 2)
	{
		overlay1.show();
		//overlay2.show();
		visualizer.setXmlParam("bandwidth", "thin");
		visualizer.setMode(1);
	}
	else if (mode == 3)
	{
		visualizer.setXmlParam("bandwidth", "thin");
		visualizer.setMode(1);
	}
	else if (mode == 4)
	{
		visualizer.setXmlParam("oscstyle", "solid");
		visualizer.setMode(2);
	}
	else if (mode == 5)
	{
		visualizer.setXmlParam("oscstyle", "dots");
		visualizer.setMode(2);
	}
	else if (mode == 6)
	{
		visualizer.setXmlParam("oscstyle", "lines");
		visualizer.setMode(2);
	}
	currentMode = mode;
}