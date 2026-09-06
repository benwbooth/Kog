#ifndef included
#error This script can only be compiled as a #include
#endif

#include "../../../scripts/attribs/gen_pageguids.m"


Function initAttribs_Playlist();

#define COSTUM_PAGE_PLAYLIST "{0167CFD9-5D35-404a-8F03-80ED5B89DEDF}"

Global ConfigAttribute playlist_search_attib;

initAttribs_Playlist(){
	initPages();
	
	ConfigItem custom_page_playlist = addConfigSubMenu(optionsmenu_page, "Playlist", COSTUM_PAGE_PLAYLIST);

	playlist_search_attib = custom_page_playlist.newAttribute("Show Playlist Search", "1");
	//addMenuSeparator(custom_page_autoresize);
}

#ifdef MAIN_ATTRIBS_MGR
/*
playlist_search_attib.onDataChanged()
{
	if (attribs_mychange) return;
	NOOFF
	attribs_mychange = 1;
	if (getData() == "1") playlist_search_attib.setData("0");
	if (getData() == "0") playlist_search_attib.setData("1");
	attribs_mychange = 0;
}*/
#endif