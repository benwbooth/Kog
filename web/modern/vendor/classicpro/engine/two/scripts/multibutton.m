#include <lib/std.mi>
#include <lib/pldir.mi>
#include <lib/config.mi>
#include "../../lib/ClassicProFile.mi"
#include <lib/application.mi>
#include <lib/quickPlaylist.mi>

Function String getMyFile();

Global Group g;
Global Popupmenu selMenu;
Global Button bolt, fakeAbout, changeTheme;

System.onScriptLoaded() {
	g = getScriptGroup();
	String param = getParam();
	bolt = g.findObject(getToken(param,";",0));
	fakeAbout = g.findObject(getToken(param,";",1));
	//changeTheme = g.findObject("Cpro.theme.next");
	changeTheme = g.findObject(getToken(param,";",2));
}

bolt.onLeftClick(){
	int a = getPublicInt("cpro2.multibutton", 0);
	
	if(a==0){
		fakeAbout.setXmlParam("action", "TOGGLE");
		fakeAbout.setXmlParam("param", "guid:{D6201408-476A-4308-BF1B-7BACA1124B12}");
		fakeAbout.leftClick();
	}
	else if(a==1){
		//System.navigateUrl(getPath(getPlayItemMetaDataString("filename")));
		ClassicProFile.exploreFile(getMyFile());
	}
	else if(a==2){
		popQuickPlaylist(System.getViewportHeight()/24, false); //was 40
	}
	else if(a==3){
		changeTheme.leftClick();
	}
	else if(a==4){
		fakeAbout.setXmlParam("param", "");
		fakeAbout.setXmlParam("action", "ML_SendTo");
		fakeAbout.leftClick();
	}
	else if(a==5){
		fakeAbout.setXmlParam("param", "");
		fakeAbout.setXmlParam("action", "trackmenu");
		fakeAbout.leftClick();
	}
	
}

String getMyFile() {
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPlayItemMetaDataString("filename");
	return output;
}

bolt.onRightButtonUp(int x, int y){
	selMenu = new PopupMenu;
	selMenu.addCommand("Multi-Button Action:", -1, 0, 1);
	selMenu.addSeparator();
	selMenu.addCommand("Change Color Theme", 3, 0, 0);
	selMenu.addCommand("Explore Folder", 1, 0, 0);
	selMenu.addCommand("Open About Winamp", 0, 0, 0);
	selMenu.addCommand("Show Quick Playlist", 2, 0, 0);
	selMenu.addCommand("Show Send to Menu", 4, 0, 0);
	selMenu.addCommand("Show Track Menu", 5, 0, 0);
	selMenu.checkCommand(getPublicInt("cpro2.multibutton", 0), 1);
	
	//int a = selMenu.popAtMouse();
	int a = selMenu.popAtXY(clientToScreenX(bolt.getLeft())+2, clientToScreenY(bolt.getTop())+26);
	// clientToScreenX(trigger.getLeft()), clientToScreenY(trigger.getTop()+26)
	
	if(a>=0){
		setPublicInt("cpro2.multibutton", a);
	}
	Complete;
	delete selMenu;
}