/*---------------------------------------------------
-----------------------------------------------------
Filename:	playlistmenu.m

Type:		maki/source
Version:	skin version 1.37
Date:		10:45 31.07.2008
Author:		Pieter Nieuwoudt aka pjn123
E-Mail:		sylvester@skinconsortium.com
Internet:	http://forums.skinconsortium.com
-----------------------------------------------------
-----------------------------------------------------

How to use it:

<SongTicker id="Songticker" x="0" y="0" h="20" w="0" relatw="1"	display="SONGTITLE"/>
<script file="playlistmenu.maki" param="Songticker;40"/>

The params:
	First param = GuiObject id
	Second param = How long must the menu be?
*/


#include <lib/std.mi>
#include <lib/config.mi>
#include <lib/pldir.mi>
#include "../../scripts/lib/quickPlaylist.mi"

Global Group myGroup;
Global GuiObject clickMe;
Global int ListLength;

System.onScriptLoaded(){
	myGroup = getScriptGroup();
	clickMe = myGroup.findObject(System.getToken(System.getParam(), ";", 0));
	ListLength = stringToInteger(System.getToken(System.getParam(), ";", 1));
	
	shuffleAtt = Config.getItemByGuid("{45F3F7C1-A6F3-4EE6-A15E-125E92FC3F8D}").getAttribute("Shuffle");
	repeatAtt = Config.getItemByGuid("{45F3F7C1-A6F3-4EE6-A15E-125E92FC3F8D}").getAttribute("Repeat");
}

clickMe.onRightButtonUp(int x, int y){
	popQuickPlaylist(System.getViewportHeight()/24, true); //ignoring the list length from cpro2
}

