/*---------------------------------------------------
-----------------------------------------------------
Filename:	fileinfo.m
Version:	3.2
Type:		maki
Date:		10. Aug. 2007 - 20:42 
Author:		Martin Poehlmann aka Deimos
E-Mail:		martin@skinconsortium.com
Internet:	www.skinconsortium.com
		www.martin.deimos.de.vu
-----------------------------------------------------
---------------------------------------------------*/

#include <lib/std.mi>
//#include attribs/init_windowpage.m

// #define DEBUG
#define FILE_NAME "fileinfo.m"
#define MINI_TAG_H_CHANGE 150
//#include <lib/debug.m>

Function loadFileInfo();
Class GuiObject LinkObject;
Class GuiObject CycleObject;
Class Group Infoline;

Global Group scriptGroup, g_cover, tagsGroup, tag_status, status_box;
Global GuiObject tag_stats_grid, status_box_grid, status_box_text;
/*Global GuiObject g_rating, g_Cover, sui, ratings_xui;
Global InfoLine g_title, g_album, g_artist, g_year, g_genre, g_track, g_publisher, g_sname, g_surl, g_albumartist, g_composer, g_format, g_disc;
Global InfoLine g_target;
Global Text t_title, t_album, t_artist, t_year, t_genre, t_track, t_publisher, t_sname, t_surl, t_composer, t_albumartist, t_format, t_disc, t_rating, ratings_text;
Global Timer cycler;
Global String stationLink;
Global Popupmenu myMenu;
Global Button optionsButton;
Global List cycle;
Global Boolean cycler_paused, quick_change;
Global LinkObject linkArtist, linkAlbum, linkTitle, linkGenre, linkPublisher, linkSURl, linkSname;
Global CycleObject cycleGenre, cyclePublisher, cycleTrack, cycleYear, cycleComposer, cycleAlbumartist, cycleFormat, cycleDisc;
Global int startwith = 0;
Global int maxlines;
Global timer delayLoad;*/
Global int _powerSave1;
Global int w2, h2;

System.onScriptLoaded()
{
	scriptGroup = getScriptGroup();

	tagsGroup = scriptGroup.getObject("info.component.infodisplay.holder");
	g_cover = scriptGroup.getObject("tagviewer.cover");
	
	tag_status = scriptGroup.getObject("tagviewer.status");
	tag_stats_grid = tag_status.getObject("tagviewer.status.grid");
	status_box = tag_status.getObject("tagviewer.status.box");
	status_box_grid = status_box.getObject("tagviewer.status.box.grid");
	status_box_text = status_box.getObject("tagviewer.status.box.text");
	
}

scriptGroup.onSetVisible(boolean onOff){
	if(onOff){
		scriptGroup.onAction("update_settings", "", 0, 0, 0, 0, scriptGroup);
	}
}


scriptGroup.onResize(int x, int y, int w, int h){
	if(!scriptGroup.isVisible()) return;

	if(getPublicInt("cpro2.tags.51", 1)==1){
		g_cover.show();
		if(w>h){
			if(_powerSave1!=1){
				g_cover.setXmlParam("relatw", "0");
				g_cover.setXmlParam("relath", "1");
				g_cover.setXmlParam("h", "-3");

				tagsGroup.setXmlParam("y", "1");
				tagsGroup.setXmlParam("h", "-20");

				tag_stats_grid.setXmlParam("topleft", "sui.status.fade");
				status_box.setXmlParam("x", "20");
				status_box.setXmlParam("w", "-85");
			
				_powerSave1=1;
			}
			
			if(h>w/2-1) w2 = w/2-1;
			else w2= h-1;
			g_cover.setXmlParam("w", integerToString(w2));

			tag_status.setXmlParam("x", integerToString(w2));
			tag_status.setXmlParam("w", integerToString(-w2));

			tagsGroup.setXmlParam("x", integerToString(w2+5));
			tagsGroup.setXmlParam("w", integerToString(w-(w2+5-8)));
			
		}
		else{
			if(_powerSave1!=2){
				g_cover.setXmlParam("relatw", "1");
				g_cover.setXmlParam("relath", "0");
				g_cover.setXmlParam("w", "-2");

				
				tagsGroup.setXmlParam("x", "1");
				tagsGroup.setXmlParam("w", "-2");

				tag_status.setXmlParam("x", "0");
				tag_status.setXmlParam("w", "0");
				tag_stats_grid.setXmlParam("topleft", "");
				status_box.setXmlParam("x", "1");
				status_box.setXmlParam("w", "-66");
			
				_powerSave1=2;
			}
			
			if(w>h/2) h2 = h/2;
			else h2 = w;
			
			g_cover.setXmlParam("h", integerToString(h2));
			tagsGroup.setXmlParam("y", integerToString(h2));
			tagsGroup.setXmlParam("h", integerToString(-h2-20));
			
			
		}
	}
	else{
		g_cover.hide();
		if(_powerSave1!=3){
				tagsGroup.setXmlParam("x", "1");
				tagsGroup.setXmlParam("w", "-2");
				tagsGroup.setXmlParam("y", integerToString(1));
				tagsGroup.setXmlParam("h", integerToString(-22));

				tag_status.setXmlParam("x", "0");
				tag_status.setXmlParam("w", "0");
				tag_stats_grid.setXmlParam("topleft", "");
				status_box.setXmlParam("x", "1");
				status_box.setXmlParam("w", "-66");
			
				_powerSave1=3;
			}
	}
	
}

scriptGroup.onAction(String action, String param, Int x, int y, int p1, int p2, GuiObject source){
	if(strlower(action)=="update_settings"){
		//_powerSave1=0; //Force update
		scriptGroup.onResize(0,0,scriptGroup.getWidth(),scriptGroup.getHeight());
		
		//Settings
		if(getPublicInt("cpro2.tags.52", 1)==1) status_box.show();
		else status_box.hide();

		//Scroll text?
		if(getPublicInt("cpro2.tags.53", 0)==1) status_box_text.setXmlParam("ticker", "1");
		else status_box_text.setXmlParam("ticker", "0");

	}
}