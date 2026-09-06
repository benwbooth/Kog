#include <lib/std.mi>

Global Group mainGroup, myGroup, file_group, play_group, options_group, view_group, help_group;
Global Layer bg_title;
Global boolean disableMenu, titleOn;

System.onScriptLoaded() {
	mainGroup = getScriptGroup();
	myGroup = mainGroup.findObject("player.mainmenu");
	bg_title = mainGroup.findObject("cpro.bg.title");

	Map temp = new Map;
	temp.loadMap("window.titlebar.menu.1");
	if(temp.getARGBValue(0,0,3)!=0){  //Winamp crash if getARGB is used on fully transparent bitmap (5.53)
		if(temp.getARGBValue(0,0,2)==255 && temp.getARGBValue(0,0,1)==0 && temp.getARGBValue(0,0,0)==128){
			myGroup.hide();
			disableMenu=true;
			
			if(!bg_title.isInvalid()){
				titleOn=true;
				bg_title.show();
			}
			
		}
	}
	delete temp;
	
	file_group = myGroup.findObject("player.mainmenu.file");
	play_group = myGroup.findObject("player.mainmenu.play");
	options_group = myGroup.findObject("player.mainmenu.options");
	view_group = myGroup.findObject("player.mainmenu.view");
	help_group = myGroup.findObject("player.mainmenu.help");
	
	play_group.setXmlParam("x", integerToString(file_group.getAutoWidth()));
	options_group.setXmlParam("x", integerToString(play_group.getLeft()+play_group.getAutoWidth()));
	view_group.setXmlParam("x", integerToString(options_group.getLeft()+options_group.getAutoWidth()));
	help_group.setXmlParam("x", integerToString(view_group.getLeft()+view_group.getAutoWidth()));
	myGroup.setXmlParam("w", integerToString(help_group.getLeft()+help_group.getAutoWidth()));
	
	myGroup.onAction("update_menu", "", 0, 0, 0, 0, myGroup);
}

mainGroup.onResize(int x, int y, int w, int h){
	if(!disableMenu){
		if(w<myGroup.getWidth()+232) myGroup.hide();
		else myGroup.show();
	}
	
	if(titleOn){
		bg_title.setXmlParam("x", integerToString(143+(w-317)/2-45));
	}
}

myGroup.onAction(String action, String param, Int x, int y, int p1, int p2, GuiObject source){
	if(action=="update_menu"){
		if(getPublicInt("cPro.menubar", 1)){
			file_group.show();
			play_group.show();
			options_group.show();
			view_group.show();
			help_group.show();
		}
		else{
			file_group.hide();
			play_group.hide();
			options_group.hide();
			view_group.hide();
			help_group.hide();
		}
	}
}